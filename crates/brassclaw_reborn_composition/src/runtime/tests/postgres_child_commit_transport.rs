//! Child-fixture byte selectors only. The actual PostgreSQL wire listener,
//! forwarding and shutdown belong to brassclaw_pg's gated native test support.
pub(super) use brassclaw_pg::test_support::AckCutTransport as ChildCommitTransport;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule};

#[derive(Clone, Copy)]
pub(super) enum ChildWrite {
    EnsureThread = 1,
    AcceptInput = 2,
    AdmitRun = 3,
}
impl ChildWrite {
    pub(super) fn rules() -> Vec<AckCutRule> {
        vec![
            AckCutRule::after_bind(
                Self::EnsureThread as u8,
                vec![
                    b"monty-child-v1-".to_vec(),
                    b"inbound_idempotency".to_vec(),
                    b"messages".to_vec(),
                ],
                vec![b"monty-child-v1:".to_vec()],
                AckBoundary::IdleReady,
            ),
            AckCutRule::after_bind(
                Self::AcceptInput as u8,
                vec![b"monty-child-v1:".to_vec(), b"inbound_idempotency".to_vec()],
                vec![b"claimed_child_reservation_released".to_vec()],
                AckBoundary::Commit,
            ),
            AckCutRule::after_bind(
                Self::AdmitRun as u8,
                vec![
                    b"monty-child-v1:".to_vec(),
                    b"claimed_child_reservation_released".to_vec(),
                ],
                vec![],
                AckBoundary::Commit,
            ),
        ]
    }
}
