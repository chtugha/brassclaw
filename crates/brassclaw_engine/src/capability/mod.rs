//! Capability management.
//!
//! - [`CapabilityRegistry`] — stores known capabilities and their actions
//! - [`LeaseManager`] — grants, validates, and expires capability leases
//! - [`PolicyEngine`] — deterministic effect-level allow/deny/approve
//! - [`DbToolSource`] — DB-backed [`ToolRegistryStore`] for `reborn_tools`
//!   (available behind the `skills-db` feature)

#[cfg(feature = "skills-db")]
pub mod db_tool_source;
pub mod lease;
pub mod planner;
pub mod policy;
pub mod registry;

#[cfg(feature = "skills-db")]
pub use db_tool_source::DbToolSource;
pub use lease::LeaseManager;
pub use policy::{PolicyDecision, PolicyEngine};
pub use registry::CapabilityRegistry;
