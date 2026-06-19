//! V1 Path Utils Module Stub
//!
//! This module provides stubs for the deleted V1 path utilities.

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

use std::path::PathBuf;

/// Stub for deleted V1 path utility function
pub fn normalize_path(_path: &str) -> String {
    String::new()
}

/// Stub for deleted V1 validate_path function
pub fn validate_path(_path: &str) -> Result<PathBuf, String> {
    Err("V1 path validation deleted".to_string())
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
