include!("event_stream_manager_contract/support/imports.rs");
include!("event_stream_manager_contract/support/fakes.rs");
include!("event_stream_manager_contract/support/builders.rs");

/// Stale event cursor position used in cursor-rebase / staleness tests.
const STALE_CURSOR: u64 = 99;

#[path = "event_stream_manager_contract/admission.rs"]
mod admission;
#[path = "event_stream_manager_contract/core.rs"]
mod core;
#[path = "event_stream_manager_contract/outbound_misc.rs"]
mod outbound_misc;
#[path = "event_stream_manager_contract/redaction_live.rs"]
mod redaction_live;
#[path = "event_stream_manager_contract/subscription.rs"]
mod subscription;
