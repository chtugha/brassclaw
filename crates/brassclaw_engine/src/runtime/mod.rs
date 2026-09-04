//! Thread runtime primitives.
//!
//! - [`messaging`] — inter-thread signal channel (`ThreadOutcome`, `signal_channel`)
//! - [`internal_write`] — self-modify guard

pub mod internal_write;
pub mod messaging;

pub use internal_write::{
    SelfModifyTestGuard, is_trusted_internal_write_active, self_modify_enabled,
    set_self_modify_for_test, with_trusted_internal_writes,
};
pub use messaging::ThreadOutcome;
