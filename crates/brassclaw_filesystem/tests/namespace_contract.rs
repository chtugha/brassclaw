#[path = "support/namespace_contract.rs"]
mod contract;

#[tokio::test]
async fn in_memory_namespace_and_cas_are_atomic() {
    contract::assert_namespace_contract(std::sync::Arc::new(
        brassclaw_filesystem::InMemoryBackend::new(),
    ))
    .await;
}
