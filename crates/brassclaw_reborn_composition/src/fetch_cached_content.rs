//! First-party handler for the `brassclaw.fetch_cached_content` tool.
//!
//! The handler reads from the `CurrentCacheBridgeSlot` which is updated each
//! turn by the `ContentCachingPortDecorator`. The model calls this tool to
//! retrieve full content or filtered sections of a previously-cached tool
//! output.

use async_trait::async_trait;
use brassclaw_host_api::{CapabilityId, HostApiError};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityHandler, FirstPartyCapabilityRegistry,
    FirstPartyCapabilityRequest, FirstPartyCapabilityResult,
};
use brassclaw_reborn::content_cache_port::CurrentCacheBridgeSlot;
use serde::Deserialize;

/// Tool capability ID.
pub(crate) const FETCH_CACHED_CONTENT_CAPABILITY_ID: &str = "brassclaw.fetch_cached_content";

/// Arguments parsed from the tool call input JSON.
#[derive(Debug, Deserialize)]
struct FetchCachedContentArgs {
    key: String,
    #[serde(default)]
    filter: Option<String>,
}

/// Register the `fetch_cached_content` handler into the first-party registry.
pub(crate) fn register_fetch_cached_content_handler(
    registry: &mut FirstPartyCapabilityRegistry,
    slot: CurrentCacheBridgeSlot,
) -> Result<(), HostApiError> {
    let handler = FetchCachedContentHandler { slot };
    registry.insert_handler(
        CapabilityId::new(FETCH_CACHED_CONTENT_CAPABILITY_ID)?,
        std::sync::Arc::new(handler),
    );
    Ok(())
}

struct FetchCachedContentHandler {
    slot: CurrentCacheBridgeSlot,
}

#[async_trait]
impl FirstPartyCapabilityHandler for FetchCachedContentHandler {
    async fn dispatch(
        &self,
        request: FirstPartyCapabilityRequest,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let args: FetchCachedContentArgs = serde_json::from_value(request.input.clone())
            .map_err(|error| {
                FirstPartyCapabilityError::with_safe_summary(
                    brassclaw_host_api::RuntimeDispatchErrorKind::InvalidResult,
                    format!("fetch_cached_content: invalid input: {error}"),
                )
            })?;

        let result = self.slot.with_current(|cache| {
            cache.fetch(&args.key, args.filter.as_deref())
        });

        let output = result.unwrap_or_else(|| {
            "Content cache is not available for this request.".to_string()
        });

        Ok(FirstPartyCapabilityResult::new(
            serde_json::Value::String(output),
            brassclaw_host_api::ResourceUsage::default(),
        ))
    }
}
