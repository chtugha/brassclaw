//! Prompt interceptor service for the Sempai–Kohai dual-role architecture.
//!
//! # What the Interceptor Does
//!
//! The interceptor sits between `PromptStage` and `ModelStage` in the
//! agent-loop executor pipeline.  It captures a complete telemetry snapshot
//! (the assembled prompt, all logical segments with their inclusion decisions,
//! token accounting, capability surface) **before** the Kohai provider sees
//! the prompt.  After the Kohai responds, the interceptor closes the packet
//! with the response + actual token usage.
//!
//! ## Routing state (no Sempai connected)
//!
//! 1. Captures the final prompt as a [`ForensicPacket`] and saves it to the
//!    [`InterceptorStore`].
//! 2. Forwards the prompt to the Kohai provider unchanged.
//! 3. Receives the Kohai response, attaches it to the packet
//!    (`status = Complete`), and saves again.
//!
//! ## Rerouting state (Sempai connected)
//!
//! 1–3 as above, but between steps 1 and 2:
//!
//! 4. Constructs a rich Sempai audit prompt containing the Kohai prompt,
//!    all segment metadata, token accounting, recipe/skill/tool context,
//!    and orchestrator design information.
//! 5. Sends the audit prompt to the Sempai provider.
//! 6. Receives [`SempaiReviewOutcome`] which contains:
//!    - An adjusted Kohai prompt (forwarded to Kohai instead of the original)
//!    - A composition summary (persisted with the packet)
//!    - Optional recipe/skill/tool updates (sent to the validation queue)
//!    - Optional agent settings adjustments
//! 7. Closes the packet as `status = SempaiReviewed` and saves.
//!
//! # Crate layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`error`] | `InterceptorError` — thiserror error type |
//! | [`mode`] | `InterceptorMode` + `SharedInterceptorMode` atomic flag |
//! | [`packet`] | `ForensicPacket`, `PacketId`, `PacketStatus`, `CapturedPrompt`, `SempaiReviewOutcome` |
//! | [`store`] | `InterceptorStore` trait + `NoopInterceptorStore` |

#![forbid(unsafe_code)]
#![warn(unreachable_pub)]

pub mod config_store;
pub mod error;
pub mod mode;
pub mod packet;
pub mod pg_store;
pub mod proposal_sink;
pub mod store;

// Convenience re-exports so callers only need to import from `brassclaw_interceptor`.
pub use config_store::{InterceptorConfig, InterceptorConfigStore};
pub use error::InterceptorError;
pub use mode::{InterceptorMode, SharedInterceptorMode};
pub use packet::{
    CapturedPrompt, ForensicPacket, KohaiUsage, PacketId, PacketStatus, PromptSegment,
    SempaiReviewOutcome, TokenAccountingSnapshot,
};
pub use pg_store::PgInterceptorStore;
pub use proposal_sink::{NoopProposalSink, ProposalSubmitResult, SempaiProposalSink};
pub use store::{InterceptorStore, NoopInterceptorStore};
