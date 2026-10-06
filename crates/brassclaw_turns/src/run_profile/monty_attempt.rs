//! Trusted worker identity for an orchestrator task attempt.
//! Lease tokens stay in Rust transport; they must never enter Python, model
//! context, serializable task metadata or diagnostic output.

use crate::{TurnLeaseToken, TurnRunId, TurnRunnerId};

/// Distinguishes retries/reclaims of the same durable run. This is an address,
/// not an authorization grant: the kernel and durable claim remain authoritative.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MontyTaskAttempt {
    pub run_id: TurnRunId,
    pub runner_id: TurnRunnerId,
    pub lease_token: TurnLeaseToken,
}

impl std::fmt::Debug for MontyTaskAttempt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MontyTaskAttempt")
            .field("run_id", &self.run_id)
            .field("runner_id", &self.runner_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_address_excludes_claim_token() {
        let attempt = MontyTaskAttempt {
            run_id: TurnRunId::new(),
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
        };
        let diagnostic = format!("{attempt:?}");
        assert!(diagnostic.contains(&attempt.run_id.to_string()));
        assert!(diagnostic.contains(&format!("{:?}", attempt.runner_id)));
        let token = serde_json::to_value(attempt.lease_token).expect("test token");
        assert!(!diagnostic.contains(token.as_str().expect("UUID token")));
    }
}
