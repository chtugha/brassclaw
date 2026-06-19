//! V1 Web Channel Module Stub
//!
//! This module provides stubs for the deleted V1 web channel system.
//! All functionality has been migrated to V2 (brassclaw-reborn).

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

pub mod platform;

use crate::error::{Error, Result};
use crate::channels::{Channel, IncomingMessage};
use async_trait::async_trait;
use futures::stream::BoxStream;

/// Stub for deleted V1 WebChannel
#[derive(Debug, Clone)]
pub struct WebChannel {
    name: String,
}

impl WebChannel {
    /// Create a new web channel stub
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Channel for WebChannel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&self) -> std::result::Result<crate::channels::MessageStream, crate::error::ChannelError> {
        Err(crate::error::ChannelError::NotSupported("V1 WebChannel deleted".to_string()))
    }

    async fn respond(
        &self,
        _msg: &IncomingMessage,
        _response: crate::channels::OutgoingResponse,
    ) -> std::result::Result<(), crate::error::ChannelError> {
        Err(crate::error::ChannelError::NotSupported("V1 WebChannel deleted".to_string()))
    }

    async fn health_check(&self) -> std::result::Result<(), crate::error::ChannelError> {
        Err(crate::error::ChannelError::NotSupported("V1 WebChannel deleted".to_string()))
    }
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
