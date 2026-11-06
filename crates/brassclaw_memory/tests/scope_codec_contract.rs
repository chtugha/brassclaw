use brassclaw_filesystem::InMemoryBackend;
use std::sync::Arc;

#[path = "support/scope_codec.rs"]
mod scope_codec;

#[tokio::test]
async fn scope_codec_preserves_ids_addresses_and_attribution_in_memory() {
    scope_codec::assert_round_trips(Arc::new(InMemoryBackend::new())).await;
}

#[tokio::test]
async fn scope_codec_rejects_noncanonical_addresses_before_writes_in_memory() {
    scope_codec::assert_invalid_addresses(Arc::new(InMemoryBackend::new())).await;
}
