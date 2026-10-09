//! Neutral, instance-wide execution limits. These grant no Tool authority and
//! carry no runtime handles, task values or platform-specific allocator state.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MontyExecutionLimits {
    pub max_source_bytes: u64,
    pub max_compiled_source_bytes: u64,
    pub max_feeds: u64,
    pub max_stdout_bytes: u64,
    pub execution_slice_millis: u64,
    pub max_value_depth: u32,
    pub max_value_nodes: u64,
    pub max_value_bytes: u64,
}

impl Default for MontyExecutionLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 1024 * 1024,
            max_compiled_source_bytes: 2 * 1024 * 1024,
            max_feeds: 128,
            max_stdout_bytes: 1024 * 1024,
            execution_slice_millis: 5,
            max_value_depth: 48,
            max_value_nodes: 1024 * 1024,
            max_value_bytes: 64 * 1024 * 1024,
        }
    }
}

impl MontyExecutionLimits {
    /// Representation-independent validation. Each host also checks its native
    /// integer, transport and control-response constraints before publication.
    pub fn validate(self) -> Result<(), &'static str> {
        if self.max_source_bytes == 0
            || self.max_compiled_source_bytes < self.max_source_bytes
            || self.max_feeds == 0
            || self.max_stdout_bytes == 0
            || self.execution_slice_millis == 0
            || self.execution_slice_millis.checked_mul(1_000_000).is_none()
            || self.max_value_depth == 0
            || self.max_value_nodes == 0
            || self.max_value_bytes == 0
        {
            return Err("invalid Monty execution limits");
        }
        Ok(())
    }
}
