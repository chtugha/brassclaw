//! V1 WASM Channel Module Stub
//!
//! This module provides stubs for the deleted V1 WASM channel system.
//! All functionality has been migrated to V2 (brassclaw-reborn).

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

use crate::error::{Error, Result};
use crate::channels::{Channel, IncomingMessage};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

/// Stub for deleted V1 SetupSchema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupSchema {
    pub fields: Vec<String>,
}

impl SetupSchema {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }
}

impl Default for SetupSchema {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for deleted V1 WasmChannel
#[derive(Debug, Clone)]
pub struct WasmChannel {
    name: String,
}

impl WasmChannel {
    /// Create a new WASM channel stub
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Load a WASM channel from bytes (stub)
    pub async fn from_bytes(_name: String, _bytes: &[u8]) -> Result<Self> {
        Err(Error::NotSupported("V1 WasmChannel deleted".to_string()))
    }
}

#[async_trait]
impl Channel for WasmChannel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&self) -> std::result::Result<crate::channels::MessageStream, crate::error::ChannelError> {
        Err(crate::error::ChannelError::NotSupported("V1 WasmChannel deleted".to_string()))
    }

    async fn respond(
        &self,
        _msg: &IncomingMessage,
        _response: crate::channels::OutgoingResponse,
    ) -> std::result::Result<(), crate::error::ChannelError> {
        Err(crate::error::ChannelError::NotSupported("V1 WasmChannel deleted".to_string()))
    }

    async fn health_check(&self) -> std::result::Result<(), crate::error::ChannelError> {
        Err(crate::error::ChannelError::NotSupported("V1 WasmChannel deleted".to_string()))
    }
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
