//! Strict post-turn response decoding. Parsing does not establish behavioral
//! correctness, association approval, Tool registration or catalogue activation.
use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    value_contract::strict_json,
};

#[derive(thiserror::Error, Debug)]
pub enum AnalysisError {
    #[error("invalid completed-turn analysis")]
    Invalid,
    #[error("unsupported proposed component")]
    Unsupported,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    format: String,
    analysis: String,
    proposals: Vec<Proposal>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    candidate_bytes: String,
    base: Option<Reference>,
    dependencies: Vec<Reference>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    uuid: String,
    class_code: i32,
    version: u64,
    checksum: String,
}
fn reference(raw: Reference) -> Result<ComponentRevisionRef, AnalysisError> {
    let uuid = Uuid::parse_str(&raw.uuid).map_err(|_| AnalysisError::Invalid)?;
    if uuid.is_nil()
        || uuid.to_string() != raw.uuid
        || raw.version == 0
        || raw.version > i64::MAX as u64
        || !matches!(raw.class_code, 0..=10 | 12..=23 | 50)
        || raw.checksum.len() != 64
        || !raw
            .checksum
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(AnalysisError::Invalid);
    }
    let mut checksum = [0; 32];
    for (slot, pair) in checksum
        .iter_mut()
        .zip(raw.checksum.as_bytes().as_chunks::<2>().0)
    {
        let digit = |c| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
        *slot = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(ComponentRevisionRef {
        uuid,
        class_code: raw.class_code,
        version: raw.version,
        checksum,
    })
}

/// Host-owned submission identity; neither the model nor the candidate can
/// choose an existing submission to overwrite. Exact retries retain the ID.
pub struct DecodedCandidate {
    pub submission_id: Uuid,
    pub candidate: ComponentRevisionDraft,
    pub base: Option<ComponentRevisionRef>,
    pub dependencies: Vec<ComponentRevisionRef>,
}
pub struct DecodedAnalysis {
    pub original_response_bytes: String,
    pub response_checksum: [u8; 32],
    pub analysis: String,
    pub candidates: Vec<DecodedCandidate>,
}

/// Fail the complete response rather than silently dropping invalid candidates.
/// Actual store submission still checks exact bases, full transitive closures,
/// retained bytes and conflicts transactionally. Protected roots and native Tool
/// implementations require separate supported authoring machinery.
pub fn decode(attempt: Uuid, bytes: &str) -> Result<DecodedAnalysis, AnalysisError> {
    if attempt.is_nil() {
        return Err(AnalysisError::Invalid);
    }
    let value = strict_json(bytes, REVISION_LIMITS).map_err(|_| AnalysisError::Invalid)?;
    // base is mandatory even when null. Serde's Option alone accepts omission.
    let proposals = value
        .get("proposals")
        .and_then(|v| v.as_array())
        .ok_or(AnalysisError::Invalid)?;
    if proposals.iter().any(|p| {
        p.as_object().is_none_or(|p| {
            !p.contains_key("base")
                || !p.contains_key("dependencies")
                || !p.contains_key("candidate_bytes")
        })
    }) {
        return Err(AnalysisError::Invalid);
    }
    let response: Response = serde_json::from_value(value).map_err(|_| AnalysisError::Invalid)?;
    if response.format != "completed-turn-sempai-analysis/1"
        || response.analysis.trim().is_empty()
        || response.proposals.len() > 4096
    {
        return Err(AnalysisError::Invalid);
    }
    let response_checksum: [u8; 32] = Sha256::digest(bytes.as_bytes()).into();
    let mut candidates = Vec::new();
    let mut identities = BTreeSet::new();
    for (index, proposal) in response.proposals.into_iter().enumerate() {
        let candidate = ComponentRevisionDraft::from_json(&proposal.candidate_bytes)
            .map_err(|_| AnalysisError::Invalid)?;
        if !matches!(candidate.class_code(),1..=3 | 13 | 21..=23) {
            return Err(AnalysisError::Unsupported);
        }
        if !identities.insert(candidate.uuid()) {
            return Err(AnalysisError::Invalid);
        }
        let base = proposal.base.map(reference).transpose()?;
        if base.is_some_and(|base| {
            base.uuid != candidate.uuid() || base.class_code != candidate.class_code()
        }) {
            return Err(AnalysisError::Invalid);
        }
        let dependencies = proposal
            .dependencies
            .into_iter()
            .map(reference)
            .collect::<Result<Vec<_>, _>>()?;
        let mut distinct = BTreeSet::new();
        if dependencies.len() > 4095
            || dependencies
                .iter()
                .any(|r| r.uuid == candidate.uuid() || !distinct.insert(r.uuid))
            || !candidate.dependencies().is_subset(&distinct)
        {
            return Err(AnalysisError::Invalid);
        }
        let mut hash = Sha256::new();
        hash.update(b"completed-turn-candidate/1\0");
        hash.update(attempt.as_bytes());
        hash.update(response_checksum);
        hash.update((index as u64).to_be_bytes());
        hash.update(candidate.checksum());
        let mut id = [0; 16];
        id.copy_from_slice(&hash.finalize()[..16]);
        id[6] = (id[6] & 0x0f) | 0x80;
        id[8] = (id[8] & 0x3f) | 0x80;
        candidates.push(DecodedCandidate {
            submission_id: Uuid::from_bytes(id),
            candidate,
            base,
            dependencies,
        });
    }
    Ok(DecodedAnalysis {
        original_response_bytes: bytes.to_owned(),
        response_checksum,
        analysis: response.analysis,
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn response(candidate: Value) -> String {
        json!({"format":"completed-turn-sempai-analysis/1","analysis":"Candidate, not proof",
            "proposals":[{"candidate_bytes":candidate.to_string(),"base":null,"dependencies":[]}]})
        .to_string()
    }
    use serde_json::Value;
    fn candidate() -> Value {
        json!({"format":"component-revision/1","uuid":Uuid::from_u128(2),"class_code":22,
            "document":{"content":"result = inputs['value']"},"dependencies":[],"association":null})
    }
    #[test]
    fn exact_response_and_ids_survive_retry_without_accepting_model_authority() {
        let bytes = response(candidate());
        let attempt = Uuid::from_u128(1);
        let a = decode(attempt, &bytes).unwrap();
        let b = decode(attempt, &bytes).unwrap();
        assert_eq!(a.original_response_bytes, bytes);
        assert_eq!(a.candidates[0].submission_id, b.candidates[0].submission_id);
        assert_ne!(
            a.candidates[0].submission_id,
            decode(Uuid::from_u128(3), &bytes).unwrap().candidates[0].submission_id
        );
        let mut forged: Value = serde_json::from_str(&bytes).unwrap();
        forged["proposals"][0]["catalogue_activated"] = json!(true);
        assert!(decode(attempt, &forged.to_string()).is_err());
        let duplicate = bytes.replacen("\"analysis\":", "\"analysis\":\"forged\",\"analysis\":", 1);
        assert!(decode(attempt, &duplicate).is_err());
    }
    #[test]
    fn empty_proposals_are_zero_candidates_and_invented_references_fail_closed() {
        let attempt = Uuid::from_u128(1);
        assert!(decode(attempt,r#"{"format":"completed-turn-sempai-analysis/1","analysis":"No reusable pattern","proposals":[]}"#).unwrap().candidates.is_empty());
        let mut value: Value = serde_json::from_str(&response(candidate())).unwrap();
        value["proposals"][0]
            .as_object_mut()
            .unwrap()
            .remove("base");
        assert!(decode(attempt, &value.to_string()).is_err());
        let mut protected = candidate();
        protected["class_code"] = json!(10);
        assert!(matches!(
            decode(attempt, &response(protected)),
            Err(AnalysisError::Unsupported)
        ));
        let mut missing = candidate();
        missing["dependencies"] = json!([Uuid::from_u128(9)]);
        assert!(decode(attempt, &response(missing)).is_err());
        let nested_duplicate = response(candidate().clone()).replace(
            "\\\"class_code\\\":22",
            "\\\"class_code\\\":21,\\\"class_code\\\":22",
        );
        assert!(decode(attempt, &nested_duplicate).is_err());
    }
}
