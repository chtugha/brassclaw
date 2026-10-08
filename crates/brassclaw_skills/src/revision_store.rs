//! Append-only instance revision retention. No legacy row is automatically
//! approved, and this store exposes no activate/delete/replace operation.

use std::{collections::BTreeSet, sync::Arc};

use brassclaw_pg::{PgError, PgPool};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{
        ComponentRevisionDraft, RetainedComponentRevision, RetainedComponentSnapshot, RevisionError,
    },
};

const MAX_SNAPSHOT_REVISIONS: usize = 4096;
const MAX_SNAPSHOT_BYTES: i64 = 67_108_864;

/// Source database diagnostics can contain authoring data. Display/Debug expose
/// classification only; an authorized diagnostic owner may inspect the source.
#[derive(thiserror::Error)]
pub enum RevisionStoreError {
    #[error("component revision database operation failed")]
    Database(#[source] PgError),
    #[error("component revision compare-and-swap conflict")]
    Conflict,
    #[error("component revision integrity failed")]
    Integrity,
    #[error("component selection requires repeatable-read or serializable isolation")]
    SnapshotIsolation,
    #[error("component snapshot exceeds technical transport capacity")]
    Capacity,
    #[error(transparent)]
    Invalid(#[from] RevisionError),
}
impl std::fmt::Debug for RevisionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}
fn database(error: impl Into<PgError>) -> RevisionStoreError {
    RevisionStoreError::Database(error.into())
}
fn digest_hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone)]
pub struct PgComponentRevisionStore {
    pool: Arc<PgPool>,
}
impl PgComponentRevisionStore {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Resolve one explicitly named historical revision for authoring/review.
    /// This is not an active selection, approval or a latest-version lookup.
    pub async fn read_revision(
        &self,
        id: Uuid,
        version: u64,
    ) -> Result<RetainedComponentRevision, RevisionStoreError> {
        if id.is_nil() || version == 0 {
            return Err(RevisionStoreError::Integrity);
        }
        let version = i64::try_from(version).map_err(|_| RevisionStoreError::Integrity)?;
        let client = self.pool.get().await.map_err(database)?;
        let row = client
            .query_opt(
                "SELECT class_code,version,revision_bytes,checksum FROM reborn_component_revisions
                 WHERE component_id=$1 AND version=$2",
                &[&id, &version],
            )
            .await
            .map_err(database)?
            .ok_or(RevisionStoreError::Integrity)?;
        let bytes: String = row.get(2);
        let revision = ComponentRevisionDraft::from_json(&bytes)?.at_version(version as u64)?;
        let actual = revision.reference();
        if actual.uuid != id
            || actual.class_code != i32::from(row.get::<_, i16>(0))
            || actual.version != row.get::<_, i64>(1) as u64
            || digest_hex(actual.checksum) != row.get::<_, String>(3)
        {
            return Err(RevisionStoreError::Integrity);
        }
        Ok(revision)
    }

    /// Version zero means a new identity. Existing edits supply the observed
    /// previous version. Concurrent edits serialize on that identity's row;
    /// losing or ambiguous calls must reread, never overwrite the winner.
    pub async fn stage(
        &self,
        draft: &ComponentRevisionDraft,
        expected_previous_version: u64,
    ) -> Result<ComponentRevisionRef, RevisionStoreError> {
        let mut client = self.pool.get().await.map_err(database)?;
        let tx = client.transaction().await.map_err(database)?;
        let reference = Self::stage_in_transaction(&tx, draft, expected_previous_version).await?;
        tx.commit().await.map_err(database)?;
        Ok(reference)
    }

    /// Retain exact packaged authoring bytes once, including after an unknown
    /// commit. This is draft retention, not trusted seed evidence or activation.
    /// A matching older revision is reused without moving the version counter;
    /// newer authored revisions and all active selections remain untouched.
    /// Ordinary edits must still use `stage` with their observed base version.
    pub async fn retain_packaged_draft(
        &self,
        draft: &ComponentRevisionDraft,
    ) -> Result<ComponentRevisionRef, RevisionStoreError> {
        let mut client = self.pool.get().await.map_err(database)?;
        let tx = client.transaction().await.map_err(database)?;
        let id = draft.uuid();
        let class = i16::try_from(draft.class_code()).map_err(|_| RevisionStoreError::Integrity)?;
        tx.execute(
            "INSERT INTO reborn_component_revision_heads (component_id,class_code)
             VALUES ($1,$2) ON CONFLICT (component_id) DO NOTHING",
            &[&id, &class],
        )
        .await
        .map_err(database)?;
        // Serialize first boot, concurrent authoring and package upgrades on the
        // existing allocation row. Read-committed observes the winner's commit.
        let head = tx
            .query_one(
                "SELECT class_code,last_version FROM reborn_component_revision_heads
                 WHERE component_id=$1 FOR UPDATE",
                &[&id],
            )
            .await
            .map_err(database)?;
        if head.get::<_, i16>(0) != class {
            return Err(RevisionStoreError::Conflict);
        }
        let previous =
            u64::try_from(head.get::<_, i64>(1)).map_err(|_| RevisionStoreError::Integrity)?;
        let checksum = digest_hex(draft.checksum());
        let existing = tx
            .query_opt(
                "SELECT class_code,version,revision_bytes,checksum
                 FROM reborn_component_revisions WHERE component_id=$1 AND checksum=$2
                 ORDER BY version LIMIT 1",
                &[&id, &checksum],
            )
            .await
            .map_err(database)?;
        let reference = if let Some(row) = existing {
            let version =
                u64::try_from(row.get::<_, i64>(1)).map_err(|_| RevisionStoreError::Integrity)?;
            if row.get::<_, i16>(0) != class
                || version == 0
                || version > previous
                || row.get::<_, String>(2) != draft.exact_bytes()
                || row.get::<_, String>(3) != checksum
            {
                return Err(RevisionStoreError::Integrity);
            }
            ComponentRevisionRef {
                uuid: id,
                class_code: i32::from(class),
                version,
                checksum: draft.checksum(),
            }
        } else {
            Self::stage_in_transaction(&tx, draft, previous).await?
        };
        tx.commit().await.map_err(database)?;
        Ok(reference)
    }

    /// Allows the trusted author/review owner to retain a revision atomically
    /// with its own supported records. This still supplies no review evidence.
    pub async fn stage_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        draft: &ComponentRevisionDraft,
        expected_previous_version: u64,
    ) -> Result<ComponentRevisionRef, RevisionStoreError> {
        let expected =
            i64::try_from(expected_previous_version).map_err(|_| RevisionStoreError::Conflict)?;
        let next = expected
            .checked_add(1)
            .ok_or(RevisionStoreError::Conflict)?;
        let id = draft.uuid();
        let class = i16::try_from(draft.class_code()).map_err(|_| RevisionStoreError::Integrity)?;
        if expected == 0 {
            tx.execute("INSERT INTO reborn_component_revision_heads (component_id,class_code) VALUES ($1,$2) ON CONFLICT (component_id) DO NOTHING", &[&id, &class]).await.map_err(database)?;
        }
        let current = tx.query_opt("SELECT class_code,last_version FROM reborn_component_revision_heads WHERE component_id=$1 FOR UPDATE", &[&id]).await.map_err(database)?
            .ok_or(RevisionStoreError::Conflict)?;
        if current.get::<_, i16>(0) != class || current.get::<_, i64>(1) != expected {
            return Err(RevisionStoreError::Conflict);
        }
        let checksum = digest_hex(draft.checksum());
        tx.execute("INSERT INTO reborn_component_revisions (component_id,class_code,version,revision_bytes,checksum) VALUES ($1,$2,$3,$4,$5)", &[&id, &class, &next, &draft.exact_bytes(), &checksum]).await.map_err(database)?;
        tx.execute(
            "UPDATE reborn_component_revision_heads SET last_version=$2 WHERE component_id=$1",
            &[&id, &next],
        )
        .await
        .map_err(database)?;
        Ok(ComponentRevisionRef {
            uuid: id,
            class_code: i32::from(class),
            version: next as u64,
            checksum: draft.checksum(),
        })
    }

    /// Read one coherent complete graph using only the caller's exact selection.
    /// Its Recipe/variant/layout document and every dependency remain retained.
    /// Missing/newer/different content is an explicit error, never latest/NoMatch.
    /// Technical bounds reject the entire graph and never truncate it.
    pub async fn read_exact(
        &self,
        roots: &[Uuid],
        references: &[ComponentRevisionRef],
    ) -> Result<RetainedComponentSnapshot, RevisionStoreError> {
        let (ids, versions) = selection_arrays(references)?;
        let mut client = self.pool.get().await.map_err(database)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(database)?;
        let snapshot = read_selected(&tx, roots, references, &ids, &versions).await?;
        tx.commit().await.map_err(database)?;
        Ok(snapshot)
    }

    /// Share the matcher's consistent catalogue view rather than opening a new
    /// transaction between matching, approval resolution and IBS assembly.
    /// The caller owns commit/rollback and all catalogue/evidence reads. This
    /// method resolves exact immutable bytes only; it does not confer approval.
    pub async fn read_exact_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        roots: &[Uuid],
        references: &[ComponentRevisionRef],
    ) -> Result<RetainedComponentSnapshot, RevisionStoreError> {
        let (ids, versions) = selection_arrays(references)?;
        let coherent: bool = tx.query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')", &[]
        ).await.map_err(database)?.get(0);
        if !coherent {
            return Err(RevisionStoreError::SnapshotIsolation);
        }
        read_selected(tx, roots, references, &ids, &versions).await
    }
}

fn selection_arrays(
    references: &[ComponentRevisionRef],
) -> Result<(Vec<Uuid>, Vec<i64>), RevisionStoreError> {
    if references.is_empty() || references.len() > MAX_SNAPSHOT_REVISIONS {
        return Err(RevisionStoreError::Capacity);
    }
    let mut unique = BTreeSet::new();
    let mut ids = Vec::with_capacity(references.len());
    let mut versions = Vec::with_capacity(references.len());
    for reference in references {
        if reference.uuid.is_nil() || reference.version == 0 || !unique.insert(reference.uuid) {
            return Err(RevisionStoreError::Integrity);
        }
        ids.push(reference.uuid);
        versions.push(i64::try_from(reference.version).map_err(|_| RevisionStoreError::Integrity)?);
    }
    Ok((ids, versions))
}

async fn read_selected(
    tx: &tokio_postgres::Transaction<'_>,
    roots: &[Uuid],
    references: &[ComponentRevisionRef],
    ids: &[Uuid],
    versions: &[i64],
) -> Result<RetainedComponentSnapshot, RevisionStoreError> {
    let totals = tx.query_one("SELECT count(*), COALESCE(sum(octet_length(r.revision_bytes)),0)::bigint FROM unnest($1::uuid[],$2::bigint[]) wanted(id,version) JOIN reborn_component_revisions r ON r.component_id=wanted.id AND r.version=wanted.version", &[&ids, &versions]).await.map_err(database)?;
    if totals.get::<_, i64>(0) != references.len() as i64 {
        return Err(RevisionStoreError::Integrity);
    }
    if totals.get::<_, i64>(1) > MAX_SNAPSHOT_BYTES {
        return Err(RevisionStoreError::Capacity);
    }
    let rows = tx.query("SELECT r.component_id,r.class_code,r.version,r.revision_bytes,r.checksum FROM unnest($1::uuid[],$2::bigint[]) wanted(id,version) JOIN reborn_component_revisions r ON r.component_id=wanted.id AND r.version=wanted.version ORDER BY r.component_id", &[&ids, &versions]).await.map_err(database)?;
    let expected: std::collections::BTreeMap<_, _> =
        references.iter().map(|r| (r.uuid, r)).collect();
    let mut retained = Vec::with_capacity(rows.len());
    for row in rows {
        let bytes: String = row.get(3);
        let draft = ComponentRevisionDraft::from_json(&bytes)?;
        let version: i64 = row.get(2);
        let revision =
            draft.at_version(u64::try_from(version).map_err(|_| RevisionStoreError::Integrity)?)?;
        let actual = revision.reference();
        if actual.uuid != row.get::<_, Uuid>(0)
            || actual.class_code != i32::from(row.get::<_, i16>(1))
            || digest_hex(actual.checksum) != row.get::<_, String>(4)
            || expected.get(&actual.uuid).copied() != Some(&actual)
        {
            return Err(RevisionStoreError::Integrity);
        }
        retained.push(revision);
    }
    Ok(RetainedComponentSnapshot::new(roots, retained)?)
}
