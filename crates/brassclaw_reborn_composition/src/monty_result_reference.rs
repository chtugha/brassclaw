//! Stable references to owner-verified internal Monty results.
use brassclaw_turns::{LoopResultRef, TurnRunId};
use sha2::{Digest, Sha256};

pub(crate) const PREFIX: &str = "result:monty.";
/// A reference correlates an opaque, owner-verified receipt with its exact run.
/// Hashing a string grants no authority; the evidence reader checks settlement.
pub(crate) fn result_reference(run: TurnRunId, receipt: &str) -> Option<LoopResultRef> {
    if receipt.is_empty() {
        return None;
    }
    LoopResultRef::new(format!(
        "{PREFIX}{run}.{}",
        hex::encode(Sha256::digest(receipt.as_bytes()))
    ))
    .ok()
}
