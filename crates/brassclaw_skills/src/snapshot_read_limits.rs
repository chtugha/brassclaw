//! Explicit graph-owner policies. These neither select nor approve a graph.
use crate::revision_store::RevisionStoreError;
use brassclaw_host_api::task_preparation::RetainedPreparation;

/// Original historical gates, not defaults for a newly captured graph.
const LEGACY_GRAPH_REVISIONS: usize = 4096;
const LEGACY_GRAPH_BYTES: u64 = 67_108_864;
#[derive(Clone, Copy)]
enum Transfer {
    Legacy,
    Captured(u64),
}
#[derive(Clone, Copy)]
pub struct SnapshotReadLimits {
    revisions: usize,
    bytes: u64,
    transfer: Transfer,
}
impl SnapshotReadLimits {
    /// Only for a retained owner admitted under the historical fixed profile.
    pub const fn legacy_graph() -> Self {
        Self {
            revisions: LEGACY_GRAPH_REVISIONS,
            bytes: LEGACY_GRAPH_BYTES,
            transfer: Transfer::Legacy,
        }
    }
    /// New-format graph admission; the caller establishes acknowledged capture
    /// and retains the original. Encoding integrity alone grants no authority.
    pub fn captured(original: &RetainedPreparation) -> Result<Self, RevisionStoreError> {
        let p = original.policy();
        p.validate().map_err(|_| RevisionStoreError::Capacity)?;
        Ok(Self {
            revisions: usize::try_from(p.max_catalogue_components_per_recipe)
                .map_err(|_| RevisionStoreError::Capacity)?,
            bytes: p.max_catalogue_bytes,
            transfer: Transfer::Captured(p.max_value_bytes),
        })
    }
    /// Old retained-policy catalogues also had the shared fixed graph gate.
    pub fn legacy_catalogue(
        p: brassclaw_host_api::MontyExecutionLimits,
    ) -> Result<Self, RevisionStoreError> {
        let mut limits = Self::legacy_graph();
        limits.revisions = limits.revisions.min(
            usize::try_from(p.max_catalogue_components_per_recipe)
                .map_err(|_| RevisionStoreError::Capacity)?,
        );
        limits.bytes = limits.bytes.min(p.max_catalogue_bytes);
        if limits.revisions == 0 || limits.bytes == 0 {
            return Err(RevisionStoreError::Capacity);
        }
        Ok(limits)
    }
    pub fn max_revisions(self) -> usize {
        self.revisions
    }
    pub fn max_revision_bytes(self) -> u64 {
        self.bytes
    }
    /// Pinned tokio-postgres unnamed typed request: Q+106+32*n bytes.
    pub(crate) fn batch_capacity(self, query: &str) -> Result<usize, RevisionStoreError> {
        let overhead = query
            .len()
            .checked_add(106)
            .ok_or(RevisionStoreError::Capacity)?;
        let budget = match self.transfer {
            Transfer::Legacy => overhead
                .checked_add(32 * LEGACY_GRAPH_REVISIONS)
                .ok_or(RevisionStoreError::Capacity)?,
            Transfer::Captured(bytes) => {
                usize::try_from(bytes).map_err(|_| RevisionStoreError::Capacity)?
            }
        };
        let count = budget
            .checked_sub(overhead)
            .ok_or(RevisionStoreError::Capacity)?
            / 32;
        let wire = brassclaw_pg::wire_capacity::binary_selection_batch_capacity(query.len())
            .ok_or(RevisionStoreError::Capacity)?;
        let count = count
            .min(wire)
            .min((i32::MAX as usize - 20) / 20)
            .min(self.revisions);
        if count == 0 {
            return Err(RevisionStoreError::Capacity);
        }
        Ok(count)
    }
    pub(crate) fn check_row(self, content_bytes: usize) -> Result<(), RevisionStoreError> {
        let row = content_bytes
            .checked_add(117)
            .ok_or(RevisionStoreError::Capacity)?;
        // Exactly maximal historical revision text still fits with its framing.
        let bound = match self.transfer {
            Transfer::Legacy => crate::component_revision::REVISION_LIMITS
                .max_bytes
                .checked_add(117)
                .ok_or(RevisionStoreError::Capacity)?,
            Transfer::Captured(bytes) => {
                usize::try_from(bytes).map_err(|_| RevisionStoreError::Capacity)?
            }
        };
        if content_bytes > crate::component_revision::REVISION_LIMITS.max_bytes
            || row > bound
            || row > i32::MAX as usize
        {
            return Err(RevisionStoreError::Capacity);
        }
        Ok(())
    }
}
