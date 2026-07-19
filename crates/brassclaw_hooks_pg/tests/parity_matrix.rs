//! Cross-backend parity matrix — Postgres + in-memory legs.
//!
//! The same deterministic scripted sequence is fed to every backend; the
//! per-step observable output is captured into an [`ObservationLog`] and
//! cross-asserted against an independent hand-computed oracle. See
//! `tests/parity_matrix/support.rs` for the assertion semantics.

#[path = "parity_matrix/oracle.rs"]
mod oracle;
#[path = "parity_matrix/scripts.rs"]
mod scripts;
#[path = "parity_matrix/support.rs"]
mod support;

use oracle::*;
use scripts::*;
use support::*;

#[tokio::test]
async fn parity_core_behavioral_script() {
    let ran = assert_parity("core", expected_core_log(), |b| async move {
        run_core_script(&*b).await
    })
    .await;
    assert!(ran.contains(&"in-memory"));
}

#[tokio::test]
async fn parity_fail_closed_cap_script() {
    let ran = assert_parity("cap", expected_cap_log(), |b| async move {
        run_cap_script(&*b).await
    })
    .await;
    assert!(ran.contains(&"in-memory"));
}

#[tokio::test]
async fn parity_per_tenant_lru_script() {
    let ran = assert_parity("lru", expected_lru_log(), |b| async move {
        run_lru_script(&*b).await
    })
    .await;
    assert!(ran.contains(&"in-memory"));
}

#[tokio::test]
async fn parity_per_tenant_lru_value_script() {
    let ran = assert_parity("lru-value", expected_lru_value_log(), |b| async move {
        run_lru_value_script(&*b).await
    })
    .await;
    assert!(ran.contains(&"in-memory"));
}

#[tokio::test]
async fn parity_global_cap_script() {
    let ran = assert_parity("global-cap", expected_global_cap_log(), |b| async move {
        run_global_cap_parity_script(&*b).await
    })
    .await;
    assert!(ran.contains(&"in-memory"));
}

#[tokio::test]
async fn parity_multisample_lru_victim_rule() {
    let ran = assert_parity(
        "multisample-lru",
        expected_multisample_lru_log(),
        |b| async move { run_multisample_lru_script(&*b).await },
    )
    .await;
    assert!(ran.contains(&"in-memory"));
}
