//! Append-only instance revision retention. No legacy row is automatically
//! approved, and this store exposes no activate/delete/replace operation.

use futures_util::{TryStreamExt, pin_mut};
use std::sync::Arc;
#[path = "snapshot_read_limits.rs"]
mod snapshot_read_limits;
pub use snapshot_read_limits::SnapshotReadLimits;

use brassclaw_host_api::task_preparation::RetainedPreparation;
use brassclaw_pg::{PgError, PgPool};
use tokio_postgres::{
    IsolationLevel,
    types::{ToSql, Type},
};
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

    /// Verify a detached original against its one immutable row in the caller's
    /// transaction. This does not select a catalogue/closure, so read-committed
    /// custody transactions are valid: the exact row cannot change between the
    /// metadata and content reads. No latest lookup or separate connection.
    pub async fn verify_original_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        original: &RetainedComponentRevision,
    ) -> Result<(), RevisionStoreError> {
        let expected = original.reference();
        let row = read_single(tx, expected.uuid, expected.version, Some(expected)).await?;
        if row.draft().exact_bytes() != original.draft().exact_bytes()
            || !row.draft().same_preparation(original.draft())
        {
            return Err(RevisionStoreError::Integrity);
        }
        Ok(())
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
        let existing = tx.query_opt(
            "SELECT class_code,version,checksum FROM reborn_component_revisions WHERE component_id=$1 AND checksum=$2 ORDER BY version LIMIT 1", &[&id,&checksum]
        ).await.map_err(database)?;
        let reference = if let Some(row) = existing {
            let version =
                u64::try_from(row.get::<_, i64>(1)).map_err(|_| RevisionStoreError::Integrity)?;
            if row.get::<_, i16>(0) != class
                || version == 0
                || version > previous
                || row.get::<_, &str>(2) != checksum
            {
                return Err(RevisionStoreError::Integrity);
            }
            // Identical package bytes keep their actual row policy, even if the
            // offered draft was prepared under newer settings. Never relabel.
            let retained = read_single(&tx, id, version, None).await?;
            if retained.draft().exact_bytes() != draft.exact_bytes() {
                return Err(RevisionStoreError::Integrity);
            }
            retained.reference()
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
        let checksum = digest_hex(draft.checksum());
        let preparation_bytes = draft.preparation().map(|p| p.bytes());
        let preparation_checksum = draft.preparation().map(|p| p.checksum());
        const INSERT: &str = "INSERT INTO reborn_component_revisions (component_id,class_code,version,revision_bytes,checksum,revision_preparation_bytes,revision_preparation_checksum) VALUES ($1,$2,$3,$4,$5,$6,$7)";
        let request_bytes = brassclaw_pg::wire_capacity::unnamed_typed_request_bytes(
            INSERT.len(),
            &[
                16,
                2,
                8,
                draft.exact_bytes().len(),
                checksum.len(),
                preparation_bytes.map_or(0, str::len),
                preparation_checksum.map_or(0, str::len),
            ],
        )
        .ok_or(RevisionStoreError::Capacity)?;
        if draft
            .preparation()
            .is_some_and(|p| request_bytes as u64 > p.policy().max_ipc_frame_bytes)
        {
            return Err(RevisionStoreError::Capacity);
        }
        if expected == 0 {
            tx.execute("INSERT INTO reborn_component_revision_heads (component_id,class_code) VALUES ($1,$2) ON CONFLICT (component_id) DO NOTHING", &[&id, &class]).await.map_err(database)?;
        }
        let current = tx.query_opt("SELECT class_code,last_version FROM reborn_component_revision_heads WHERE component_id=$1 FOR UPDATE", &[&id]).await.map_err(database)?
            .ok_or(RevisionStoreError::Conflict)?;
        if current.get::<_, i16>(0) != class || current.get::<_, i64>(1) != expected {
            return Err(RevisionStoreError::Conflict);
        }
        let exact_bytes = draft.exact_bytes();
        let params: [(&(dyn ToSql + Sync), Type); 7] = [
            (&id, Type::UUID),
            (&class, Type::INT2),
            (&next, Type::INT8),
            (&exact_bytes, Type::TEXT),
            (&checksum, Type::TEXT),
            (&preparation_bytes, Type::TEXT),
            (&preparation_checksum, Type::TEXT),
        ];
        let rows = tx.query_typed_raw(INSERT, params).await.map_err(database)?;
        pin_mut!(rows);
        while rows.try_next().await.map_err(database)?.is_some() {}
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

type SelectionArrays = (Vec<Uuid>, Vec<i64>, Vec<usize>);

fn selection_arrays(
    references: &[ComponentRevisionRef],
    limits: SnapshotReadLimits,
) -> Result<SelectionArrays, RevisionStoreError> {
    if references.is_empty() || references.len() > limits.max_revisions() {
        return Err(RevisionStoreError::Capacity);
    }
    // Reject an impossible transfer before allocating whole-graph indices.
    limits.batch_capacity(&metadata_query()?)?;
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
fn metadata_query() -> Result<String, RevisionStoreError> {
    Ok(format!(
        "SELECT r.component_id,r.class_code,r.version,octet_length(r.revision_bytes),r.checksum,{} FROM unnest($1::uuid[],$2::bigint[]) wanted(id,version) JOIN reborn_component_revisions r ON r.component_id=wanted.id AND r.version=wanted.version ORDER BY r.component_id",
        preparation_columns("r.")?
    ))
}
fn preparation_columns(prefix: &str) -> Result<String, RevisionStoreError> {
    let capacity = RetainedPreparation::maximum_canonical_bytes()
        .map_err(|_| RevisionStoreError::Integrity)?;
    Ok(format!(
        "({prefix}revision_preparation_bytes IS NOT NULL OR {prefix}revision_preparation_checksum IS NOT NULL), CASE WHEN octet_length({prefix}revision_preparation_bytes)<={capacity} THEN {prefix}revision_preparation_bytes END, CASE WHEN octet_length({prefix}revision_preparation_checksum)=64 THEN {prefix}revision_preparation_checksum END"
    ))
}
fn preparation_from_row(
    row: &tokio_postgres::Row,
    index: usize,
) -> Result<Option<RetainedPreparation>, RevisionStoreError> {
    let present: bool = row.get(index);
    let bytes: Option<&str> = row.get(index + 1);
    let checksum: Option<&str> = row.get(index + 2);
    preparation_from_parts(present, bytes, checksum)
}
fn preparation_from_parts(
    present: bool,
    bytes: Option<&str>,
    checksum: Option<&str>,
) -> Result<Option<RetainedPreparation>, RevisionStoreError> {
    match (present, bytes, checksum) {
        (false, None, None) => Ok(None),
        (true, Some(bytes), Some(checksum)) => {
            let original = RetainedPreparation::restore_original(bytes, checksum)
                .map_err(|_| RevisionStoreError::Integrity)?;
            crate::component_revision::revision_limits(Some(&original))?;
            Ok(Some(original))
        }
        _ => Err(RevisionStoreError::Integrity),
    }
}
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
    let meta_query = metadata_query()?;
    let capacity = limits
        .batch_capacity(&meta_query)?
        .min(limits.batch_capacity(CONTENT_QUERY)?);
    let mut lengths = Vec::new();
    lengths
        .try_reserve_exact(references.len())
        .map_err(|_| RevisionStoreError::Capacity)?;
    let mut preparations = Vec::new();
    preparations
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
            .query_typed_raw(&meta_query, params)
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
            let carrier: Option<&str> = row.get(6);
            let carrier_hash: Option<&str> = row.get(7);
            let metadata_bytes = brassclaw_pg::wire_capacity::binary_data_row_bytes(&[
                Some(16),
                Some(2),
                Some(8),
                Some(4),
                Some(row.get::<_, &str>(4).len()),
                Some(1),
                carrier.map(str::len),
                carrier_hash.map(str::len),
            ])
            .ok_or(RevisionStoreError::Capacity)?;
            limits.check_transfer_row(metadata_bytes)?;
            let preparation = preparation_from_row(&row, 5)?;
            limits.check_revision_row(length, preparation.as_ref())?;
            preparations.push(preparation);
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
            let revision = ComponentRevisionDraft::from_json_with_preparation(
                bytes,
                preparations[consumed].as_ref(),
            )?
            .at_version(expected.version)?;
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
    let query = format!(
        "SELECT class_code,version,octet_length(revision_bytes),checksum,{} FROM reborn_component_revisions WHERE component_id=$1 AND version=$2",
        preparation_columns("")?
    );
    let metadata = tx
        .query_opt(&query, &[&id, &version])
        .await
        .map_err(database)?
        .ok_or(RevisionStoreError::Integrity)?;
    let length =
        usize::try_from(metadata.get::<_, i32>(2)).map_err(|_| RevisionStoreError::Integrity)?;
    let preparation = preparation_from_row(&metadata, 4)?;
    let parser = crate::component_revision::revision_limits(preparation.as_ref())?;
    let carrier: Option<&str> = metadata.get(5);
    let carrier_hash: Option<&str> = metadata.get(6);
    let metadata_bytes = brassclaw_pg::wire_capacity::binary_data_row_bytes(&[
        Some(2),
        Some(8),
        Some(4),
        Some(metadata.get::<_, &str>(3).len()),
        Some(1),
        carrier.map(str::len),
        carrier_hash.map(str::len),
    ])
    .ok_or(RevisionStoreError::Capacity)?;
    if preparation
        .as_ref()
        .is_some_and(|p| metadata_bytes as u64 > p.policy().max_ipc_frame_bytes)
    {
        return Err(RevisionStoreError::Capacity);
    }
    let row_bytes = brassclaw_pg::wire_capacity::binary_data_row_bytes(&[Some(length)])
        .ok_or(RevisionStoreError::Capacity)?;
    if length > parser.max_bytes
        || preparation
            .as_ref()
            .is_some_and(|p| row_bytes as u64 > p.policy().max_ipc_frame_bytes)
    {
        return Err(RevisionStoreError::Capacity);
    }
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
    let revision = ComponentRevisionDraft::from_json_with_preparation(bytes, preparation.as_ref())?
        .at_version(version as u64)?;
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

#[cfg(test)]
mod original_policy_reader_tests {
    use super::*;
    #[test]
    fn only_actual_null_null_is_legacy_even_when_bounded_projection_hides_corruption() {
        let columns = preparation_columns("r.").unwrap();
        assert!(columns.starts_with("(r.revision_preparation_bytes IS NOT NULL OR r.revision_preparation_checksum IS NOT NULL),"));
        assert!(preparation_from_parts(false, None, None).unwrap().is_none());
        // A non-NULL short checksum is hidden by CASE but retains presence.
        assert!(matches!(
            preparation_from_parts(true, None, None),
            Err(RevisionStoreError::Integrity)
        ));
        assert!(matches!(
            preparation_from_parts(true, Some("partial"), None),
            Err(RevisionStoreError::Integrity)
        ));
        assert!(matches!(
            preparation_from_parts(true, None, Some("short")),
            Err(RevisionStoreError::Integrity)
        ));
    }
}
