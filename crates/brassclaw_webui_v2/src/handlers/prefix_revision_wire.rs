//! Browser-only revision encoding. Never use these projections in native Tools
//! or retained records; their shared DTO/Skill contracts remain numeric.
use super::WebUiV2HttpError;
use brassclaw_product_workflow::{
    PrefixGenerationDetail, PrefixGenerationSummary, PrefixJobDetail, PrefixJobTarget,
    PrefixListResponse, PrefixModelDesignCatalogue, PrefixModelSpecification,
};
use serde::Serialize;
use serde_json::Value;

type Result = std::result::Result<Value, WebUiV2HttpError>;
fn encode(value: &impl Serialize) -> Result {
    serde_json::to_value(value).map_err(WebUiV2HttpError::internal)
}
fn field(value: &mut Value, name: &str) -> std::result::Result<(), WebUiV2HttpError> {
    let slot = value
        .get_mut(name)
        .ok_or_else(|| WebUiV2HttpError::internal("missing revision field"))?;
    let revision = slot
        .as_u64()
        .ok_or_else(|| WebUiV2HttpError::internal("invalid revision field"))?;
    *slot = Value::String(revision.to_string());
    Ok(())
}
fn entries(value: &mut Value) -> std::result::Result<&mut Vec<Value>, WebUiV2HttpError> {
    value
        .as_array_mut()
        .ok_or_else(|| WebUiV2HttpError::internal("invalid revision response array"))
}
pub(super) fn model_catalogue(value: &PrefixModelDesignCatalogue) -> Result {
    let mut wire = encode(value)?;
    for name in ["targets", "specifications"] {
        for item in entries(&mut wire[name])? {
            field(item, "provider_revision")?;
        }
    }
    Ok(wire)
}
pub(super) fn model_specification(value: &PrefixModelSpecification) -> Result {
    let mut wire = encode(value)?;
    field(&mut wire, "provider_revision")?;
    Ok(wire)
}
pub(super) fn prefix_list(value: &PrefixListResponse) -> Result {
    let mut wire = encode(value)?;
    for item in entries(&mut wire["prefixes"])? {
        field(item, "profile_revision")?;
    }
    Ok(wire)
}
pub(super) fn generation_list(value: &[PrefixGenerationSummary]) -> Result {
    let mut wire = encode(&value)?;
    for item in entries(&mut wire)? {
        field(item, "profile_revision")?;
    }
    Ok(wire)
}
pub(super) fn generation_detail(value: &PrefixGenerationDetail) -> Result {
    let mut wire = encode(value)?;
    field(&mut wire["summary"], "profile_revision")?;
    Ok(wire)
}
pub(super) fn job(value: &PrefixJobDetail) -> Result {
    let mut wire = encode(value)?;
    if matches!(&value.target, PrefixJobTarget::Profile { .. }) {
        field(&mut wire, "profile_revision")?;
    }
    Ok(wire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_product_workflow::{PrefixEntry, PrefixModelTarget};
    use serde_json::json;

    #[test]
    fn http_revisions_are_exact_without_changing_shared_numeric_or_original_text() {
        for revision in [(1_u64 << 53) + 1, i64::MAX as u64, u64::MAX] {
            let original =
                " {\"provider_revision\":18446744073709551615}\r\n<script>literal</script>";
            let spec = PrefixModelSpecification {
                id: "spec".into(),
                provider_id: "p".into(),
                provider_revision: revision,
                model: "m".into(),
                specification_bytes: original.into(),
                qualification: "unknown-model-artifacts".into(),
            };
            let catalogue = PrefixModelDesignCatalogue {
                targets: vec![PrefixModelTarget {
                    provider_id: "p".into(),
                    provider_revision: revision,
                    model: "m".into(),
                    consumer: "kohai".into(),
                    configured_context_window: Some(42),
                }],
                specifications: vec![spec.clone()],
                proposals: vec![],
                pair_proposals: vec![],
                artifact_sets: vec![],
                design_generation_available: false,
                blocked_reason: None,
            };
            let wire = model_catalogue(&catalogue).unwrap();
            assert_eq!(
                wire["targets"][0]["provider_revision"],
                revision.to_string()
            );
            assert_eq!(
                wire["specifications"][0]["provider_revision"],
                revision.to_string()
            );
            assert_eq!(wire["targets"][0]["configured_context_window"], 42);
            assert_eq!(wire["specifications"][0]["specification_bytes"], original);
            assert_eq!(
                model_specification(&spec).unwrap()["provider_revision"],
                revision.to_string()
            );
            assert_eq!(
                serde_json::to_value(&spec).unwrap()["provider_revision"],
                json!(revision)
            );
            let entry = PrefixEntry {
                label: "l".into(),
                description: "d".into(),
                profile_revision: revision,
                generation_available: false,
                generation_status: "s".into(),
                blocked_reason: None,
                name: "p".into(),
                fingerprint: None,
                is_stale: false,
                assembled_at: None,
                prewarm_last_at: None,
                generation_ms: Some(1),
            };
            let list = PrefixListResponse {
                prefixes: vec![entry],
            };
            assert_eq!(
                prefix_list(&list).unwrap()["prefixes"][0]["profile_revision"],
                revision.to_string()
            );
            assert_eq!(
                serde_json::to_value(&list).unwrap()["prefixes"][0]["profile_revision"],
                json!(revision)
            );
            let summary = PrefixGenerationSummary {
                profile: "p".into(),
                generation: "g".into(),
                profile_revision: revision,
                kind: "compiler-package".into(),
                qualification: "quarantined".into(),
                created_at: "date".into(),
            };
            let detail = PrefixGenerationDetail {
                summary,
                generation_bytes: original.into(),
            };
            assert_eq!(
                generation_list(std::slice::from_ref(&detail.summary)).unwrap()[0]["profile_revision"],
                revision.to_string()
            );
            let wire = generation_detail(&detail).unwrap();
            assert_eq!(wire["summary"]["profile_revision"], revision.to_string());
            assert_eq!(wire["generation_bytes"], original);
            assert_eq!(
                serde_json::to_value(&detail).unwrap()["summary"]["profile_revision"],
                json!(revision)
            );
            let mut status = PrefixJobDetail {
                job_id: "j".into(),
                thread_id: "t".into(),
                target: PrefixJobTarget::Profile {
                    profile: "p".into(),
                    profile_revision: revision,
                },
                operation: "measure".into(),
                mode: "internal".into(),
                created_at: "date".into(),
                request_sha256: "r".into(),
                program_sha256: "p".into(),
                input_sha256: "i".into(),
                run_status: None,
                admission_phase: None,
                answered_tool_calls: 2,
                pending_tool_calls: 0,
                failure_kind: None,
                internal_receipt: Some(original.into()),
                result_ref: None,
            };
            let wire = job(&status).unwrap();
            assert_eq!(wire["profile_revision"], revision.to_string());
            assert_eq!(wire["answered_tool_calls"], 2);
            assert_eq!(wire["internal_receipt"], original);
            assert_eq!(
                serde_json::to_value(&status).unwrap()["profile_revision"],
                json!(revision)
            );
            status.target = PrefixJobTarget::Pair {
                specification: "s".into(),
                proposal: "p".into(),
            };
            assert_eq!(
                job(&status).unwrap(),
                serde_json::to_value(&status).unwrap()
            );
        }
    }
}
