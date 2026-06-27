//! Skills system for BrassClaw.
//!
//! This module contains main-crate skill logic that depends on types from the
//! extracted `brassclaw_llm` crate (e.g. `brassclaw_llm::ToolDefinition`) and
//! other `src/` modules (e.g. `crate::secrets`). For core skill types,
//! parsing, and registry, import from `brassclaw_skills` directly.
//!
//! The `attenuation` submodule lives here because it operates on
//! `brassclaw_llm::ToolDefinition` together with main-crate trust state, so it
//! sits at the seam between the two.
//!
pub mod bundled;

// Items from `brassclaw_skills` are no longer glob-re-exported.
// Callers should import from `brassclaw_skills` directly.

use crate::secrets::{CredentialLocation, CredentialMapping};
use brassclaw_skills::{LoadedSkill, SkillCredentialLocation, SkillCredentialSpec};
// CredentialMapping is used in credential_spec_to_mapping

/// Convert a skill credential location to the main crate's [`CredentialLocation`].
fn convert_credential_location(loc: &SkillCredentialLocation) -> CredentialLocation {
    match loc {
        SkillCredentialLocation::Bearer => CredentialLocation::AuthorizationBearer,
        SkillCredentialLocation::BasicAuth { username } => CredentialLocation::AuthorizationBasic {
            username: username.clone(),
        },
        SkillCredentialLocation::Header { name, prefix } => CredentialLocation::Header {
            name: name.clone(),
            prefix: prefix.clone(),
        },
        SkillCredentialLocation::QueryParam { name } => {
            CredentialLocation::QueryParam { name: name.clone() }
        }
    }
}

/// Convert a [`SkillCredentialSpec`] to a [`CredentialMapping`].
pub fn credential_spec_to_mapping(spec: &SkillCredentialSpec) -> CredentialMapping {
    CredentialMapping {
        secret_name: spec.name.clone(),
        location: convert_credential_location(&spec.location),
        host_patterns: spec.hosts.clone(),
        path_patterns: spec.path_patterns.clone(),
        // Skill credentials are required by default; the spec doesn't yet
        // expose an `optional` field, so we conservatively mark required.
        optional: false,
    }
}

/// Validate and log credential mappings from loaded skills.
///
/// Validates each spec; invalid specs are logged and skipped.
pub fn register_skill_credentials(skills: &[LoadedSkill]) {
    let mut count = 0usize;
    for skill in skills {
        for spec in &skill.manifest.credentials {
            let errors = brassclaw_skills::validation::validate_credential_spec(spec);
            if !errors.is_empty() {
                tracing::warn!(
                    skill = %skill.name(),
                    credential = %spec.name,
                    errors = ?errors,
                    "Skipping invalid credential spec"
                );
                continue;
            }
            let _mapping = credential_spec_to_mapping(spec);
            tracing::debug!(
                skill = %skill.name(),
                credential = %spec.name,
                hosts = ?spec.hosts,
                "Registering skill credential mapping"
            );
            count += 1;
        }
    }
    if count > 0 {
        tracing::debug!(count, "Registered skill credential mappings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_bearer_location() {
        let loc = brassclaw_skills::SkillCredentialLocation::Bearer;
        let converted = convert_credential_location(&loc);
        assert!(matches!(
            converted,
            crate::secrets::CredentialLocation::AuthorizationBearer
        ));
    }

    #[test]
    fn test_convert_basic_auth_location() {
        let loc = brassclaw_skills::SkillCredentialLocation::BasicAuth {
            username: "admin".to_string(),
        };
        let converted = convert_credential_location(&loc);
        match converted {
            crate::secrets::CredentialLocation::AuthorizationBasic { username } => {
                assert_eq!(username, "admin");
            }
            _ => panic!("expected AuthorizationBasic"),
        }
    }

    #[test]
    fn test_convert_header_location() {
        let loc = brassclaw_skills::SkillCredentialLocation::Header {
            name: "X-API-Key".to_string(),
            prefix: Some("Token".to_string()),
        };
        let converted = convert_credential_location(&loc);
        match converted {
            crate::secrets::CredentialLocation::Header { name, prefix } => {
                assert_eq!(name, "X-API-Key");
                assert_eq!(prefix, Some("Token".to_string()));
            }
            _ => panic!("expected Header"),
        }
    }

    #[test]
    fn test_convert_query_param_location() {
        let loc = brassclaw_skills::SkillCredentialLocation::QueryParam {
            name: "key".to_string(),
        };
        let converted = convert_credential_location(&loc);
        match converted {
            crate::secrets::CredentialLocation::QueryParam { name } => {
                assert_eq!(name, "key");
            }
            _ => panic!("expected QueryParam"),
        }
    }

    #[test]
    fn test_credential_spec_to_mapping() {
        let spec = brassclaw_skills::SkillCredentialSpec {
            name: "github_token".to_string(),
            provider: "github".to_string(),
            location: brassclaw_skills::SkillCredentialLocation::Bearer,
            hosts: vec!["api.github.com".to_string(), "*.github.com".to_string()],
            path_patterns: Vec::new(),
            oauth: None,
            setup_instructions: None,
        };
        let mapping = super::credential_spec_to_mapping(&spec);
        assert_eq!(mapping.secret_name, "github_token");
        assert!(matches!(
            mapping.location,
            crate::secrets::CredentialLocation::AuthorizationBearer
        ));
        assert_eq!(mapping.host_patterns.len(), 2);
        assert_eq!(mapping.host_patterns[0], "api.github.com");
    }
}
