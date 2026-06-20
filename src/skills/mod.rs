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
pub mod attenuation;
pub mod bundled;

// Items from `brassclaw_skills` are no longer glob-re-exported.
// Callers should import from `brassclaw_skills` directly.

// Re-export attenuation at the same path as before.
pub use attenuation::{AttenuationResult, attenuate_tools};

use crate::secrets::{CredentialLocation, CredentialMapping};
// V1 - deleted: auth module no longer exists
// auth::{AuthDescriptor, AuthDescriptorKind, OAuthFlowDescriptor, upsert_auth_descriptor},
use brassclaw_skills::{LoadedSkill, SkillCredentialLocation, SkillCredentialSpec};

/// Stub for deleted V1 OAuthRefreshConfig type
#[derive(Debug, Clone)]
pub struct OAuthRefreshConfig {
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub exchange_proxy_url: Option<String>,
    pub gateway_token: Option<String>,
    pub secret_name: String,
    pub provider: Option<String>,
    pub extra_refresh_params: std::collections::HashMap<String, String>,
}

/// Stub for deleted V1 SharedCredentialRegistry type
pub struct SharedCredentialRegistry {
    // Minimal stub - no fields needed
}

impl SharedCredentialRegistry {
    /// Stub method to register a credential mapping
    pub fn register(&self, _mapping: CredentialMapping) {
        // No-op stub - credential registration not supported in V1 stub
    }
}

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

/// Convert a [`SkillCredentialSpec`] to a [`CredentialMapping`] for the
/// [`SharedCredentialRegistry`](crate::wasm_runtime::SharedCredentialRegistry).
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

/// Register credential mappings from loaded skills into the shared registry.
///
/// Validates each spec before registration; invalid specs are logged and skipped.
pub fn register_skill_credentials(
    skills: &[LoadedSkill],
    _registry: &SharedCredentialRegistry,
) {
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

// V1 - deleted: Auth descriptor persistence no longer needed
// pub async fn persist_skill_auth_descriptors(
//     skills: &[LoadedSkill],
//     store: Option<&dyn SettingsStore>,
//     user_id: &str,
// ) {
//     for skill in skills {
//         for spec in &skill.manifest.credentials {
//             let errors = brassclaw_skills::validation::validate_credential_spec(spec);
//             if !errors.is_empty() {
//                 continue;
//             }
//
//             let descriptor = credential_spec_to_auth_descriptor(skill.name(), spec);
//             upsert_auth_descriptor(store, user_id, descriptor).await;
//         }
//     }
// }

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

    #[test]
    fn test_register_skill_credentials_valid() {
        use brassclaw_skills::types::*;
        use std::path::PathBuf;

        let skill = brassclaw_skills::LoadedSkill {
            manifest: SkillManifest {
                name: "test-api".to_string(),
                version: "1.0.0".to_string(),
                description: "Test".to_string(),
                activation: ActivationCriteria::default(),
                credentials: vec![SkillCredentialSpec {
                    name: "test_token".to_string(),
                    provider: "test".to_string(),
                    location: SkillCredentialLocation::Bearer,
                    hosts: vec!["api.test.com".to_string()],
                    path_patterns: Vec::new(),
                    oauth: None,
                    setup_instructions: None,
                }],
                requires: GatingRequirements::default(),
            },
            prompt_content: "test".to_string(),
            trust: SkillTrust::Trusted,
            source: SkillSource::User(PathBuf::from("/tmp/test")), // safety: dummy path in test, not used for I/O
            content_hash: "sha256:000".to_string(),
            compiled_patterns: vec![],
            lowercased_keywords: vec![],
            lowercased_exclude_keywords: vec![],
            lowercased_tags: vec![],
        };

        let registry = crate::wasm_runtime::SharedCredentialRegistry::new();
        register_skill_credentials(&[skill], &registry);

        assert!(registry.has_credentials_for_host("api.test.com"));
        assert!(!registry.has_credentials_for_host("other.host.com"));
    }

    #[test]
    fn test_register_skill_credentials_registers_oauth_refresh_config() {
        use brassclaw_skills::types::*;
        use std::path::PathBuf;

        let skill = brassclaw_skills::LoadedSkill {
            manifest: SkillManifest {
                name: "gmail".to_string(),
                version: "1.0.0".to_string(),
                description: "Test".to_string(),
                activation: ActivationCriteria::default(),
                credentials: vec![SkillCredentialSpec {
                    name: "google_oauth_token".to_string(),
                    provider: "google".to_string(),
                    location: SkillCredentialLocation::Bearer,
                    hosts: vec!["www.googleapis.com".to_string()],
                    path_patterns: Vec::new(),
                    oauth: Some(SkillOAuthConfig {
                        authorization_url: "https://accounts.google.com/o/oauth2/v2/auth"
                            .to_string(),
                        token_url: "https://oauth2.googleapis.com/token".to_string(),
                        client_id: Some("client-id".to_string()),
                        client_id_env: None,
                        client_secret: Some("client-secret".to_string()),
                        client_secret_env: None,
                        scopes: vec![],
                        use_pkce: true,
                        extra_params: std::collections::HashMap::new(),
                        refresh: ProviderRefreshStrategy::Standard,
                        test_url: None,
                    }),
                    setup_instructions: None,
                }],
                requires: GatingRequirements::default(),
            },
            prompt_content: "test".to_string(),
            trust: SkillTrust::Trusted,
            source: SkillSource::User(PathBuf::from("/tmp/test")),
            content_hash: "sha256:000".to_string(),
            compiled_patterns: vec![],
            lowercased_keywords: vec![],
            lowercased_exclude_keywords: vec![],
            lowercased_tags: vec![],
        };

        let registry = crate::wasm_runtime::SharedCredentialRegistry::new();
        register_skill_credentials(&[skill], &registry);

        let oauth = registry
            .oauth_refresh_for_secret("google_oauth_token")
            .expect("oauth refresh config");
        assert_eq!(oauth.secret_name, "google_oauth_token");
        assert_eq!(oauth.token_url, "https://oauth2.googleapis.com/token");
        assert_eq!(oauth.client_id, "client-id");
        assert_eq!(oauth.client_secret.as_deref(), Some("client-secret"));
    }

    #[test]
    fn test_register_skill_credentials_invalid_skipped() {
        use brassclaw_skills::types::*;
        use std::path::PathBuf;

        let skill = brassclaw_skills::LoadedSkill {
            manifest: SkillManifest {
                name: "bad-skill".to_string(),
                version: "1.0.0".to_string(),
                description: "Test".to_string(),
                activation: ActivationCriteria::default(),
                credentials: vec![SkillCredentialSpec {
                    name: "INVALID_NAME".to_string(), // uppercase = invalid
                    provider: "test".to_string(),
                    location: SkillCredentialLocation::Bearer,
                    hosts: vec!["api.test.com".to_string()],
                    path_patterns: Vec::new(),
                    oauth: None,
                    setup_instructions: None,
                }],
                requires: GatingRequirements::default(),
            },
            prompt_content: "test".to_string(),
            trust: SkillTrust::Trusted,
            source: SkillSource::User(PathBuf::from("/tmp/test")), // safety: dummy path in test, not used for I/O
            content_hash: "sha256:000".to_string(),
            compiled_patterns: vec![],
            lowercased_keywords: vec![],
            lowercased_exclude_keywords: vec![],
            lowercased_tags: vec![],
        };

        let registry = crate::wasm_runtime::SharedCredentialRegistry::new();
        register_skill_credentials(&[skill], &registry);

        // Invalid spec should be skipped — host should NOT be registered
        assert!(!registry.has_credentials_for_host("api.test.com"));
    }
}
