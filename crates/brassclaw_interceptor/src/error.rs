//! Error type for the interceptor service.

use thiserror::Error;

/// Errors that can occur within the interceptor service.
#[derive(Debug, Error)]
pub enum InterceptorError {
    #[error("interceptor store unavailable: {reason}")]
    StoreUnavailable { reason: String },

    #[error("forensic packet not found: {packet_id}")]
    PacketNotFound { packet_id: String },

    #[error("sempai provider unavailable: {reason}")]
    SempaiUnavailable { reason: String },

    #[error("sempai response malformed: {reason}")]
    SempaiResponseMalformed { reason: String },

    #[error("interceptor store internal error: {reason}")]
    Internal { reason: String },
}
