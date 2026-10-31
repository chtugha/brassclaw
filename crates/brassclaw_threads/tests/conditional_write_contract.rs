//! Actual consumer operations must retain exact CAS through projection adaptation.
#[path = "../../brassclaw_filesystem/tests/support/conditional_write_backend.rs"]
mod conditional_backend;
#[path = "support/conditional_write_contract.rs"]
mod contract;

#[tokio::test]
async fn conditional_writes_preserve_originals_and_supported_transitions() {
    contract::verify(std::sync::Arc::new(
        brassclaw_filesystem::InMemoryBackend::new(),
    ))
    .await;
}
