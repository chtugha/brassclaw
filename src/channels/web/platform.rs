//! V1 Web Platform Module Stub
//!
//! This module provides stubs for the deleted V1 web platform system.

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

pub mod state;

use serde::{Deserialize, Serialize};

/// Stub for deleted V1 WebPlatform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPlatform {
    pub name: String,
}

impl WebPlatform {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
