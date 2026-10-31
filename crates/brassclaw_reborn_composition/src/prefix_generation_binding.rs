//! Immutable target artifacts inside the existing generation package. These are
//! integrity facts, never approval. No duplicate prefix registry or setting store.
use super::prefix_model_designs::{bad, hash};
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_model_encoding::checksum;
use brassclaw_product_workflow::InterceptorConfigServiceError as Error;
use brassclaw_skills::value_contract::strict_json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

pub(crate) const BINDING_FILE: &str = "model_input_binding.json";
pub(crate) const PREFIX_FILE: &str = "prefix.utf8";
pub(crate) const TARGET_FORMAT: &str = "brassclaw-prefix-target-generation/1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GenerationBinding {
    format: String,
    pub(crate) specification: String,
    pub(crate) artifact_set: String,
    pub(crate) pair_proposal: String,
    pub(crate) profile_proposal: String,
    pub(crate) mapping_sha256: String,
    prefix_file: String,
    prefix_sha256: String,
}
pub(crate) struct TargetPrefixArtifact {
    pub(crate) binding: GenerationBinding,
    pub(crate) prefix: Arc<str>,
    pub(crate) reference: Arc<str>,
    pub(crate) source_generation: String,
    pub(crate) source_package_bytes: String,
}
/// First authenticate the whole existing compiler package and all of its files.
/// The sidecar has no own-generation or future-approval ID, avoiding hash cycles.
/// Complete source originals stay present and validated by validate_package().
pub(crate) fn read(
    profile: &str,
    generation: &str,
    original: &str,
) -> Result<TargetPrefixArtifact, Error> {
    hash(generation)?;
    if checksum(original.as_bytes()) != generation {
        return Err(bad());
    }
    let package = strict_json(original, super::prefix_generations::LIMITS).map_err(|_| bad())?;
    if package.as_object().is_none_or(|p| p.len() != 5)
        || package["format"] != TARGET_FORMAT
        || package["profile"] != profile
        || package["artifacts"]
            .as_object()
            .is_none_or(|p| p.len() != 2)
    {
        return Err(bad());
    }
    let source_generation = package["source_generation"].as_str().ok_or_else(bad)?;
    hash(source_generation)?;
    let source_package_bytes = package["source_package_bytes"].as_str().ok_or_else(bad)?;
    if checksum(source_package_bytes.as_bytes()) != source_generation {
        return Err(bad());
    }
    let source =
        strict_json(source_package_bytes, super::prefix_generations::LIMITS).map_err(|_| bad())?;
    // Target envelopes cannot nest or reinterpret a legacy serving bundle.
    // Preserve the complete original source compiler package and its truthful
    // source-only/non-activation manifest. Target qualification is independent.
    if source["format"] != "brassclaw-prefix-generation-package/1" {
        return Err(bad());
    }
    super::prefix_generations::validate_package(profile, source_package_bytes)?;
    let artifact = |package: &Value, name: &str| -> Result<Vec<u8>, Error> {
        let file = &package["artifacts"][name];
        if file.as_object().is_none_or(|f| f.len() != 2) || file["encoding"] != "base64" {
            return Err(bad());
        }
        let encoded = file["data"].as_str().ok_or_else(bad)?;
        STANDARD.decode(encoded).map_err(|_| bad())
    };
    let bytes = artifact(&package, BINDING_FILE)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| bad())?;
    let data: Value =
        strict_json(text, super::prefix_model_designs::PROPOSAL_LIMITS).map_err(|_| bad())?;
    let binding: GenerationBinding = serde_json::from_value(data).map_err(|_| bad())?;
    if binding.format != "brassclaw-prefix-model-input-binding/1"
        || binding.prefix_file != PREFIX_FILE
    {
        return Err(bad());
    }
    for v in [
        &binding.specification,
        &binding.artifact_set,
        &binding.pair_proposal,
        &binding.profile_proposal,
        &binding.mapping_sha256,
        &binding.prefix_sha256,
    ] {
        hash(v)?;
    }
    let bytes = artifact(&package, PREFIX_FILE)?;
    if checksum(&bytes) != binding.prefix_sha256 {
        return Err(bad());
    }
    let prefix = String::from_utf8(bytes).map_err(|_| bad())?;
    if prefix.is_empty() {
        return Err(bad());
    }
    Ok(TargetPrefixArtifact {
        binding,
        prefix: Arc::from(prefix),
        reference: Arc::from(
            String::from_utf8(artifact(&source, "context.md")?).map_err(|_| bad())?,
        ),
        source_generation: source_generation.into(),
        source_package_bytes: source_package_bytes.into(),
    })
}
impl TargetPrefixArtifact {
    /// Compare to resolved records, not operator strings or mutable latest rows.
    pub(crate) fn require_references(
        &self,
        specification: &str,
        artifact_set: &str,
        pair_proposal: &str,
        profile_proposal: &str,
        mapping_json: &str,
    ) -> Result<(), Error> {
        let b = &self.binding;
        if b.specification != specification
            || b.artifact_set != artifact_set
            || b.pair_proposal != pair_proposal
            || b.profile_proposal != profile_proposal
            || b.mapping_sha256 != checksum(mapping_json.as_bytes())
        {
            return Err(bad());
        }
        Ok(())
    }
}
