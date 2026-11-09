//! Append-only instance revision retention. No legacy row is automatically
//! approved, and this store exposes no activate/delete/replace operation.

use futures_util::{TryStreamExt, pin_mut};
use std::sync::Arc;
#[path = "snapshot_read_limits.rs"]
mod snapshot_read_limits;
pub use snapshot_read_limits::SnapshotReadLimits;

use brassclaw_pg::{PgError, PgPool};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{
        ComponentRevisionDraft, RetainedComponentRevision, RetainedComponentSnapshot, RevisionError,
    },
};

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
    #[error("component snapshot exceeds its original resource or transport policy")]
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

    /// Allocation metadata only. This does not select the latest revision or
    /// establish approval/activation. Unknown identities have no allocated head.
    pub async fn read_allocation_head(
        &self,
        id: Uuid,
    ) -> Result<Option<(i32, u64)>, RevisionStoreError> {
        if id.is_nil() {
            return Err(RevisionStoreError::Integrity);
        }
        let client = self.pool.get().await.map_err(database)?;
        let row=client.query_opt("SELECT class_code,last_version FROM reborn_component_revision_heads WHERE component_id=$1",&[&id]).await.map_err(database)?;
        row.map(|row| {
            Ok((
                i32::from(row.get::<_, i16>(0)),
                u64::try_from(row.get::<_, i64>(1)).map_err(|_| RevisionStoreError::Integrity)?,
            ))
        })
        .transpose()
    }

    /// Resolve one explicitly named historical revision for authoring/review.
    /// This is not an active selection, approval or a latest-version lookup.
    pub async fn read_revision(
        &self,
        id: Uuid,
        version: u64,
    ) -> Result<RetainedComponentRevision, RevisionStoreError> {
        if id.is_nil() || version == 0 || i64::try_from(version).is_err() {
            return Err(RevisionStoreError::Integrity);
        }
        let mut client = brassclaw_pg::transaction_client::TransactionClient::from_client(
            self.pool.get().await.map_err(database)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(database)?;
        let revision = read_single(&tx, id, version, None).await?;
        tx.commit().await.map_err(database)?;
        client.recycle_after_commit();
        Ok(revision)
    }
    /// Exact root comparison; no reconstruction of its historical closure.
    pub async fn read_revision_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        expected: ComponentRevisionRef,
    ) -> Result<RetainedComponentRevision, RevisionStoreError> {
        require_snapshot(tx).await?;
        read_single(tx, expected.uuid, expected.version, Some(expected)).await
    }

    /// Version zero means a new identity. Existing edits supply the observed
    /// previous version. Concurrent edits serialize on that identity's row;
    /// losing or ambiguous calls must reread, never overwrite the winner.
    pub async fn stage(
        &self,
        draft: &ComponentRevisionDraft,
        expected_previous_version: u64,
    ) -> Result<ComponentRevisionRef, RevisionStoreError> {
        let mut client = brassclaw_pg::transaction_client::TransactionClient::from_client(
            self.pool.get().await.map_err(database)?,
        );
        let tx = client.transaction().await.map_err(database)?;
        let reference = Self::stage_in_transaction(&tx, draft, expected_previous_version).await?;
        tx.commit().await.map_err(database)?;
        client.recycle_after_commit();
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
        let mut client = brassclaw_pg::transaction_client::TransactionClient::from_client(
            self.pool.get().await.map_err(database)?,
        );
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
        client.recycle_after_commit();
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
        limits: SnapshotReadLimits,
    ) -> Result<RetainedComponentSnapshot, RevisionStoreError> {
        let (ids, versions, order) = selection_arrays(references, limits)?;
        let mut client = brassclaw_pg::transaction_client::TransactionClient::from_client(
            self.pool.get().await.map_err(database)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(database)?;
        let snapshot =
            read_selected(&tx, roots, references, &ids, &versions, &order, limits).await?;
        tx.commit().await.map_err(database)?;
        client.recycle_after_commit();
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
        limits: SnapshotReadLimits,
    ) -> Result<RetainedComponentSnapshot, RevisionStoreError> {
        let (ids, versions, order) = selection_arrays(references, limits)?;
        require_snapshot(tx).await?;
        read_selected(tx, roots, references, &ids, &versions, &order, limits).await
    }
}

async fn require_snapshot(tx: &tokio_postgres::Transaction<'_>) -> Result<(), RevisionStoreError> {
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(database)?
        .get(0);
    if !coherent {
        return Err(RevisionStoreError::SnapshotIsolation);
    }
    Ok(())
}

fn selection_arrays(
    references: &[ComponentRevisionRef],
    limits: SnapshotReadLimits,
) -> Result<(Vec<Uuid>, Vec<i64>, Vec<usize>), RevisionStoreError> {
    if references.is_empty() || references.len() > limits.max_revisions() {
        return Err(RevisionStoreError::Capacity);
    }
    // Reject an impossible transfer before allocating whole-graph indices.
    limits.batch_capacity(META_QUERY)?;
    limits.batch_capacity(CONTENT_QUERY)?;
    let mut order = Vec::new();
    order
        .try_reserve_exact(references.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    order.extend(0..references.len());
    order.sort_unstable_by_key(|i| references[*i].uuid);
    if order
        .windows(2)
        .any(|pair| references[pair[0]].uuid == references[pair[1]].uuid)
    {
        return Err(RevisionStoreError::Integrity);
    }
    let mut ids = Vec::new();
    ids.try_reserve_exact(references.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    let mut versions = Vec::new();
    versions
        .try_reserve_exact(references.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    for reference in references {
        if reference.uuid.is_nil() || reference.version == 0 {
            return Err(RevisionStoreError::Integrity);
        }
        ids.push(reference.uuid);
        versions.push(i64::try_from(reference.version).map_err(|_| RevisionStoreError::Integrity)?);
    }
    Ok((ids, versions, order))
}

// Query text is fixed; values remain typed parameters. The actual pinned wire
// encoder uses two arrays regardless of graph count (see snapshot_read_limits).
const META_QUERY: &str = "SELECT r.component_id,r.class_code,r.version,octet_length(r.revision_bytes),r.checksum FROM unnest($1::uuid[],$2::bigint[]) wanted(id,version) JOIN reborn_component_revisions r ON r.component_id=wanted.id AND r.version=wanted.version ORDER BY r.component_id";
const CONTENT_QUERY: &str = "SELECT r.component_id,r.class_code,r.version,r.revision_bytes,r.checksum FROM unnest($1::uuid[],$2::bigint[]) wanted(id,version) JOIN reborn_component_revisions r ON r.component_id=wanted.id AND r.version=wanted.version ORDER BY r.component_id";
async fn read_selected(
    tx: &tokio_postgres::Transaction<'_>,
    roots: &[Uuid],
    references: &[ComponentRevisionRef],
    ids: &[Uuid],
    versions: &[i64],
    order: &[usize],
    limits: SnapshotReadLimits,
) -> Result<RetainedComponentSnapshot, RevisionStoreError> {
    use tokio_postgres::types::{ToSql, Type};
    let capacity = limits
        .batch_capacity(META_QUERY)?
        .min(limits.batch_capacity(CONTENT_QUERY)?);
    let mut lengths = Vec::new();
    lengths
        .try_reserve_exact(references.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    let mut total = 0_u64;
    // Finish metadata for the ENTIRE selected set before requesting any body.
    for batch in order.chunks(capacity) {
        let (batch_ids, batch_versions) = batch_arrays(batch, ids, versions)?;
        let params: [(&(dyn ToSql + Sync), Type); 2] = [
            (&batch_ids, Type::UUID_ARRAY),
            (&batch_versions, Type::INT8_ARRAY),
        ];
        let rows = tx
            .query_typed_raw(META_QUERY, params)
            .await
            .map_err(database)?;
        pin_mut!(rows);
        let mut seen = 0;
        while let Some(row) = rows.try_next().await.map_err(database)? {
            let index = *batch.get(seen).ok_or(RevisionStoreError::Integrity)?;
            let expected = references[index];
            if row.get::<_, Uuid>(0) != expected.uuid
                || i32::from(row.get::<_, i16>(1)) != expected.class_code
                || row.get::<_, i64>(2) != versions[index]
                || row.get::<_, &str>(4) != digest_hex(expected.checksum)
            {
                return Err(RevisionStoreError::Integrity);
            }
            let length =
                usize::try_from(row.get::<_, i32>(3)).map_err(|_| RevisionStoreError::Integrity)?;
            limits.check_row(length)?;
            total = total
                .checked_add(u64::try_from(length).map_err(|_| RevisionStoreError::Capacity)?)
                .ok_or(RevisionStoreError::Capacity)?;
            if total > limits.max_revision_bytes() {
                return Err(RevisionStoreError::Capacity);
            }
            lengths.push(length);
            seen += 1;
        }
        if seen != batch.len() {
            return Err(RevisionStoreError::Integrity);
        }
        tracing::debug!(target: "brassclaw::snapshot_read", phase = "metadata", revisions = seen, "completed component snapshot query batch");
    }
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(references.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    let mut consumed = 0;
    for batch in order.chunks(capacity) {
        let (batch_ids, batch_versions) = batch_arrays(batch, ids, versions)?;
        let params: [(&(dyn ToSql + Sync), Type); 2] = [
            (&batch_ids, Type::UUID_ARRAY),
            (&batch_versions, Type::INT8_ARRAY),
        ];
        let rows = tx
            .query_typed_raw(CONTENT_QUERY, params)
            .await
            .map_err(database)?;
        pin_mut!(rows);
        let mut seen = 0;
        while let Some(row) = rows.try_next().await.map_err(database)? {
            let index = *batch.get(seen).ok_or(RevisionStoreError::Integrity)?;
            let expected = references[index];
            let bytes: &str = row.get(3);
            if bytes.len() != lengths[consumed]
                || row.get::<_, Uuid>(0) != expected.uuid
                || i32::from(row.get::<_, i16>(1)) != expected.class_code
                || row.get::<_, i64>(2) != versions[index]
                || row.get::<_, &str>(4) != digest_hex(expected.checksum)
            {
                return Err(RevisionStoreError::Integrity);
            }
            let revision =
                ComponentRevisionDraft::from_json(bytes)?.at_version(expected.version)?;
            if revision.reference() != expected {
                return Err(RevisionStoreError::Integrity);
            }
            retained.push(revision);
            consumed += 1;
            seen += 1;
        }
        if seen != batch.len() {
            return Err(RevisionStoreError::Integrity);
        }
        tracing::debug!(target: "brassclaw::snapshot_read", phase = "content", revisions = seen, "completed component snapshot query batch");
    }
    if consumed != references.len() {
        return Err(RevisionStoreError::Integrity);
    }
    Ok(RetainedComponentSnapshot::new(roots, retained)?)
}
fn batch_arrays(
    batch: &[usize],
    ids: &[Uuid],
    versions: &[i64],
) -> Result<(Vec<Uuid>, Vec<i64>), RevisionStoreError> {
    let mut selected_ids = Vec::new();
    let mut selected_versions = Vec::new();
    selected_ids
        .try_reserve_exact(batch.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    selected_versions
        .try_reserve_exact(batch.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    for index in batch {
        selected_ids.push(ids[*index]);
        selected_versions.push(versions[*index]);
    }
    Ok((selected_ids, selected_versions))
}

async fn read_single(
    tx: &tokio_postgres::Transaction<'_>,
    id: Uuid,
    version: u64,
    expected: Option<ComponentRevisionRef>,
) -> Result<RetainedComponentRevision, RevisionStoreError> {
    if id.is_nil() || version == 0 {
        return Err(RevisionStoreError::Integrity);
    }
    let version = i64::try_from(version).map_err(|_| RevisionStoreError::Integrity)?;
    let metadata = tx.query_opt("SELECT class_code,version,octet_length(revision_bytes),checksum FROM reborn_component_revisions WHERE component_id=$1 AND version=$2", &[&id,&version]).await.map_err(database)?.ok_or(RevisionStoreError::Integrity)?;
    let length =
        usize::try_from(metadata.get::<_, i32>(2)).map_err(|_| RevisionStoreError::Integrity)?;
    SnapshotReadLimits::legacy_graph().check_row(length)?;
    let class = i32::from(metadata.get::<_, i16>(0));
    let checksum: &str = metadata.get(3);
    if metadata.get::<_, i64>(1) != version
        || expected.is_some_and(|r| r.class_code != class || digest_hex(r.checksum) != checksum)
    {
        return Err(RevisionStoreError::Integrity);
    }
    let row = tx.query_opt("SELECT revision_bytes FROM reborn_component_revisions WHERE component_id=$1 AND version=$2", &[&id,&version]).await.map_err(database)?.ok_or(RevisionStoreError::Integrity)?;
    let bytes: &str = row.get(0);
    if bytes.len() != length {
        return Err(RevisionStoreError::Integrity);
    }
    let revision = ComponentRevisionDraft::from_json(bytes)?.at_version(version as u64)?;
    let actual = revision.reference();
    if actual.uuid != id
        || actual.class_code != class
        || digest_hex(actual.checksum) != checksum
        || expected.is_some_and(|r| r != actual)
    {
        return Err(RevisionStoreError::Integrity);
    }
    Ok(revision)
}
