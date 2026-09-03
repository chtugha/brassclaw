//! Mode-driven security configuration (Step C.4).
//!
//! Defined at the loop layer (`brassclaw_turns`) so that:
//! 1. The C.6 cross-turn driver can resolve the per-turn security posture
//!    through [`crate::run_profile::LoopSecurityPort`] without depending on the
//!    composition layer's DB-backed store.
//! 2. The composition layer (`brassclaw_reborn_composition`) — the sole crate
//!    depending on both the engine and the agent-loop stack — provides the
//!    [`SecurityConfigSource`] impl backed by the `reborn_security_settings`
//!    table (migration V068).
//!
//! This mirrors the established `RetrievalLookup` / `LoopRetrievalPort`
//! crate-boundary discipline: the trait + DTOs live here (turns-native), the
//! DB-backed impl lives in composition, and the production host holds an
//! `Option<Arc<dyn SecurityConfigSource>>` threaded in via a builder.
//!
//! # Two modes (Fork3=A)
//!
//! The per-turn [`SecurityMode`] is auto-detected from `host.resolve_intent`:
//! - **Matching** — an intent matched a Q2+ validated component. Every loaded
//!   component is trusted (it passed the kohai/sempai validation queue), so the
//!   runtime wrapper is OFF: the recipe executes as intended. `event_emission`
//!   stays ON (observability, not a security gate).
//! - **NonMatching** — no intent matched, so an LLM is involved. The wrapper is
//!   ON for every layer.
//!
//! The operator can force any of the six layers on or off regardless of mode
//! via a per-layer [`SecurityLayerOverride`] (`Auto` / `ForceOn` / `ForceOff`)
//! stored in `reborn_security_settings` and surfaced in the WebUI security
//! panel. `Auto` defers to the mode-driven default.

use std::fmt;
use std::str::FromStr;

use async_trait::async_trait;

/// The per-turn security mode, auto-detected from `host.resolve_intent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    /// Intent matched a Q2+ validated component — wrapper OFF (trusted path).
    Matching,
    /// No intent matched, an LLM is involved — wrapper ON.
    NonMatching,
}

/// The six individually-toggleable wrapper layers (Fork1=A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLayer {
    /// PolicyEngine — capability policy.
    Policy,
    /// LeaseManager — capability leases.
    Leases,
    /// GateController — approval gates.
    Gate,
    /// Event emission — observability (default ON in both modes; not a gate).
    EventEmission,
    /// Sensitive-tool self-scoping.
    SensitiveToolScoping,
    /// Bind-time namespace filtering (LLM path only).
    NamespaceFiltering,
}

/// All six security layers, in canonical order.
pub const ALL_SECURITY_LAYERS: [SecurityLayer; 6] = [
    SecurityLayer::Policy,
    SecurityLayer::Leases,
    SecurityLayer::Gate,
    SecurityLayer::EventEmission,
    SecurityLayer::SensitiveToolScoping,
    SecurityLayer::NamespaceFiltering,
];

/// A per-layer operator override. Stored as DB `TEXT` (`'auto'`/`'on'`/`'off'`)
/// and surfaced in the WebUI panel as a three-state toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SecurityLayerOverride {
    /// Defer to the mode-driven default (Matching OFF / Non-Matching ON, except
    /// `event_emission` which is ON in both).
    #[serde(rename = "auto")]
    Auto,
    /// Force the layer ON regardless of mode.
    #[serde(rename = "on")]
    ForceOn,
    /// Force the layer OFF regardless of mode.
    #[serde(rename = "off")]
    ForceOff,
}

impl SecurityLayerOverride {
    /// The DB `TEXT` representation (`'auto'`/`'on'`/`'off'`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ForceOn => "on",
            Self::ForceOff => "off",
        }
    }
}

impl fmt::Display for SecurityLayerOverride {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SecurityLayerOverride {
    type Err = SecurityConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "on" => Ok(Self::ForceOn),
            "off" => Ok(Self::ForceOff),
            other => Err(SecurityConfigError::Deserialize(format!(
                "invalid security override '{other}' (expected auto|on|off)"
            ))),
        }
    }
}

/// The resolved on/off state of all six layers for a given mode — the concrete
/// shape the C.6 driver consults before each turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedSecurityLayers {
    pub policy: bool,
    pub leases: bool,
    pub gate: bool,
    pub event_emission: bool,
    pub sensitive_tool_scoping: bool,
    pub namespace_filtering: bool,
}

impl ResolvedSecurityLayers {
    /// Read the resolved state of a single layer.
    pub fn is_active(&self, layer: SecurityLayer) -> bool {
        match layer {
            SecurityLayer::Policy => self.policy,
            SecurityLayer::Leases => self.leases,
            SecurityLayer::Gate => self.gate,
            SecurityLayer::EventEmission => self.event_emission,
            SecurityLayer::SensitiveToolScoping => self.sensitive_tool_scoping,
            SecurityLayer::NamespaceFiltering => self.namespace_filtering,
        }
    }
}

/// The operator-level security posture: one [`SecurityLayerOverride`] per layer.
///
/// `default()` is all `Auto` (mode-driven). The composition store returns this
/// for a tenant with no `reborn_security_settings` row (no DB-less mode —
/// Postgres is always used); the WebUI PUT upserts the row on first save.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SecurityModeConfig {
    pub policy_override: SecurityLayerOverride,
    pub leases_override: SecurityLayerOverride,
    pub gate_override: SecurityLayerOverride,
    pub event_emission_override: SecurityLayerOverride,
    pub sensitive_tool_scoping_override: SecurityLayerOverride,
    pub namespace_filtering_override: SecurityLayerOverride,
}

impl Default for SecurityModeConfig {
    fn default() -> Self {
        Self {
            policy_override: SecurityLayerOverride::Auto,
            leases_override: SecurityLayerOverride::Auto,
            gate_override: SecurityLayerOverride::Auto,
            event_emission_override: SecurityLayerOverride::Auto,
            sensitive_tool_scoping_override: SecurityLayerOverride::Auto,
            namespace_filtering_override: SecurityLayerOverride::Auto,
        }
    }
}

impl SecurityModeConfig {
    /// The override configured for a given layer.
    pub fn override_for(&self, layer: SecurityLayer) -> SecurityLayerOverride {
        match layer {
            SecurityLayer::Policy => self.policy_override,
            SecurityLayer::Leases => self.leases_override,
            SecurityLayer::Gate => self.gate_override,
            SecurityLayer::EventEmission => self.event_emission_override,
            SecurityLayer::SensitiveToolScoping => self.sensitive_tool_scoping_override,
            SecurityLayer::NamespaceFiltering => self.namespace_filtering_override,
        }
    }

    /// Resolve a single layer's active state for the given mode.
    ///
    /// `ForceOn`/`ForceOff` win regardless of mode; `Auto` defers to the
    /// mode-driven default ([`Self::auto_default`]).
    pub fn resolve(&self, layer: SecurityLayer, mode: SecurityMode) -> bool {
        match self.override_for(layer) {
            SecurityLayerOverride::ForceOn => true,
            SecurityLayerOverride::ForceOff => false,
            SecurityLayerOverride::Auto => Self::auto_default(layer, mode),
        }
    }

    /// Resolve every layer for the given mode into a [`ResolvedSecurityLayers`].
    pub fn resolve_all(&self, mode: SecurityMode) -> ResolvedSecurityLayers {
        ResolvedSecurityLayers {
            policy: self.resolve(SecurityLayer::Policy, mode),
            leases: self.resolve(SecurityLayer::Leases, mode),
            gate: self.resolve(SecurityLayer::Gate, mode),
            event_emission: self.resolve(SecurityLayer::EventEmission, mode),
            sensitive_tool_scoping: self.resolve(SecurityLayer::SensitiveToolScoping, mode),
            namespace_filtering: self.resolve(SecurityLayer::NamespaceFiltering, mode),
        }
    }

    /// Mode-driven default for a layer (the `Auto` resolution).
    ///
    /// - `EventEmission` is ON in both modes (observability, not a gate).
    /// - Every other layer is OFF in Matching mode (trusted Q2+ path) and ON in
    ///   Non-Matching mode (an LLM is involved).
    fn auto_default(layer: SecurityLayer, mode: SecurityMode) -> bool {
        match (layer, mode) {
            (SecurityLayer::EventEmission, _) => true,
            (SecurityLayer::Policy, SecurityMode::NonMatching) => true,
            (SecurityLayer::Leases, SecurityMode::NonMatching) => true,
            (SecurityLayer::Gate, SecurityMode::NonMatching) => true,
            (SecurityLayer::SensitiveToolScoping, SecurityMode::NonMatching) => true,
            (SecurityLayer::NamespaceFiltering, SecurityMode::NonMatching) => true,
            (SecurityLayer::Policy, SecurityMode::Matching) => false,
            (SecurityLayer::Leases, SecurityMode::Matching) => false,
            (SecurityLayer::Gate, SecurityMode::Matching) => false,
            (SecurityLayer::SensitiveToolScoping, SecurityMode::Matching) => false,
            (SecurityLayer::NamespaceFiltering, SecurityMode::Matching) => false,
        }
    }
}

/// Errors raised by security-config lookups. Mirrors the
/// `RetrievalLookupError` shape (manual `Display`/`Error` impls — no
/// `thiserror` dependency in this module).
#[derive(Debug)]
pub enum SecurityConfigError {
    /// Backend failed to load the config row (DB error, etc.).
    Load(String),
    /// A stored override value did not parse to `auto|on|off`.
    Deserialize(String),
}

impl fmt::Display for SecurityConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(reason) => write!(f, "security config load error: {reason}"),
            Self::Deserialize(reason) => write!(f, "security config deserialize error: {reason}"),
        }
    }
}

impl std::error::Error for SecurityConfigError {}

/// DB-backed security-config source. The composition layer implements this over
/// the `reborn_security_settings` table (V068); the C.6 driver calls it through
/// [`crate::run_profile::LoopSecurityPort`] at the start of each turn.
#[async_trait]
pub trait SecurityConfigSource: Send + Sync {
    /// Load the operator-level [`SecurityModeConfig`] for the current tenant.
    /// A missing row yields [`SecurityModeConfig::default()`] (all `Auto`).
    async fn load_config(&self) -> Result<SecurityModeConfig, SecurityConfigError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_all_auto() {
        let cfg = SecurityModeConfig::default();
        for layer in ALL_SECURITY_LAYERS {
            assert_eq!(cfg.override_for(layer), SecurityLayerOverride::Auto, "{layer:?}");
        }
    }

    #[test]
    fn resolve_all_matching_uses_matching_defaults() {
        let resolved = SecurityModeConfig::default().resolve_all(SecurityMode::Matching);
        // event_emission is ON in both modes; everything else OFF in Matching.
        assert!(!resolved.policy);
        assert!(!resolved.leases);
        assert!(!resolved.gate);
        assert!(resolved.event_emission);
        assert!(!resolved.sensitive_tool_scoping);
        assert!(!resolved.namespace_filtering);
    }

    #[test]
    fn resolve_all_non_matching_uses_non_matching_defaults() {
        let resolved = SecurityModeConfig::default().resolve_all(SecurityMode::NonMatching);
        // Every layer ON in Non-Matching.
        assert!(resolved.policy);
        assert!(resolved.leases);
        assert!(resolved.gate);
        assert!(resolved.event_emission);
        assert!(resolved.sensitive_tool_scoping);
        assert!(resolved.namespace_filtering);
    }

    #[test]
    fn force_on_overrides_mode() {
        let cfg = SecurityModeConfig {
            policy_override: SecurityLayerOverride::ForceOn,
            ..Default::default()
        };
        // Matching would default policy OFF; ForceOn wins.
        assert!(cfg.resolve(SecurityLayer::Policy, SecurityMode::Matching));
    }

    #[test]
    fn force_off_overrides_mode() {
        let cfg = SecurityModeConfig {
            gate_override: SecurityLayerOverride::ForceOff,
            ..Default::default()
        };
        // Non-Matching would default gate ON; ForceOff wins.
        assert!(!cfg.resolve(SecurityLayer::Gate, SecurityMode::NonMatching));
    }

    #[test]
    fn override_str_round_trip() {
        for ov in [
            SecurityLayerOverride::Auto,
            SecurityLayerOverride::ForceOn,
            SecurityLayerOverride::ForceOff,
        ] {
            let s = ov.as_str();
            assert_eq!(SecurityLayerOverride::from_str(s).unwrap(), ov);
        }
        assert!(SecurityLayerOverride::from_str("bogus").is_err());
    }

    #[test]
    fn config_serde_round_trip() {
        let cfg = SecurityModeConfig {
            policy_override: SecurityLayerOverride::ForceOn,
            leases_override: SecurityLayerOverride::Auto,
            gate_override: SecurityLayerOverride::ForceOff,
            event_emission_override: SecurityLayerOverride::ForceOn,
            sensitive_tool_scoping_override: SecurityLayerOverride::Auto,
            namespace_filtering_override: SecurityLayerOverride::ForceOff,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SecurityModeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
        // The wire form uses the DB TEXT tokens.
        assert!(json.contains("\"policy_override\":\"on\""));
        assert!(json.contains("\"gate_override\":\"off\""));
        assert!(json.contains("\"leases_override\":\"auto\""));
    }

    #[test]
    fn resolved_is_active_accessor() {
        let resolved = ResolvedSecurityLayers {
            policy: true,
            leases: false,
            gate: true,
            event_emission: true,
            sensitive_tool_scoping: false,
            namespace_filtering: true,
        };
        assert!(resolved.is_active(SecurityLayer::Policy));
        assert!(!resolved.is_active(SecurityLayer::Leases));
        assert!(resolved.is_active(SecurityLayer::Gate));
        assert!(resolved.is_active(SecurityLayer::EventEmission));
        assert!(!resolved.is_active(SecurityLayer::SensitiveToolScoping));
        assert!(resolved.is_active(SecurityLayer::NamespaceFiltering));
    }

    #[test]
    fn all_security_layers_has_six_entries() {
        assert_eq!(ALL_SECURITY_LAYERS.len(), 6);
    }
}
