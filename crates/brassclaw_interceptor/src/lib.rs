//! Prompt interceptor service for the Sempai–Kohai dual-role architecture.
//!
//! # What the Interceptor Does
//!
//! Reborn captures the authorized, resolved host request immediately before
//! provider dispatch. Its reference-only executor hook cannot supply prompt
//! text. Packets contain the actual messages and available segment/accounting
//! metadata; unknown provider limits remain unknown. After a real response,
//! the packet records its structured output and reported usage.
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
//! 5. Checks current model policy, reserves budget in an isolated namespace
//!    sharing the task governor, and sends the audit to the Sempai provider.
//! 6. Receives [`SempaiReviewOutcome`] which contains:
//!    - An adjusted Kohai prompt (forwarded to Kohai instead of the original)
//!    - A composition summary (persisted with the packet)
//!    - Optional recipe/skill/tool updates (sent to the validation queue)
//!    - Optional agent settings adjustments
//! 7. Records the review, forwards the validated prompt, then closes the packet
//!    as `status = SempaiReviewed` after the actual Kohai response. Review errors
//!    stop dispatch; selected System messages and typed tool replay are preserved.
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
    CapturedPrompt, ComponentProposal, ForensicPacket, KohaiUsage, PacketId, PacketStatus,
    PromptSegment, SempaiReviewOutcome, TokenAccountingSnapshot,
};
pub use pg_store::PgInterceptorStore;
pub use proposal_sink::{NoopProposalSink, ProposalSubmitResult, SempaiProposalSink};
pub use store::{InterceptorStore, NoopInterceptorStore};
