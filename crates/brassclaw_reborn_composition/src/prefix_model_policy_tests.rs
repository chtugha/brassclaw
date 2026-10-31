//! Actual PostgreSQL snapshot/provider-owner binding. No inference, duplicate
//! settings owner, workflow approval or final serving guard is exercised here.
use super::*;
use crate::{
    pg_provider_repo::ProviderDefinitionRevision, runtime::test_pg::native_pg::NativePostgres,
};

const NAMESPACE: &str = "prefix-policy-original";
const MODEL: &str = "same-model";

async fn fixture() -> (
    NativePostgres,
    PgProviderRepo,
    ProviderDefinitionRevision,
    PgPrefixModelDesigns,
) {
    let rig = NativePostgres::start().await;
    let repo = PgProviderRepo::new(rig.pool.as_ref().clone(), NAMESPACE);
    repo.initialize().await.unwrap();
    let definition = serde_json::from_value(json!({
        "id":"policy-input", "aliases":[], "protocol":"open_ai_completions",
        "default_base_url":"http://127.0.0.1:1/v1", "base_url_required":true,
        "api_key_required":false, "model_env":"UNUSED_PREFIX_POLICY_FIXTURE",
        "default_model":MODEL, "description":"Original policy fixture",
        "context_window_tokens":4,
        "model_token_limits":{MODEL:{"max_prefix_tokens":1,"max_prompt_tokens":2},
            "other-model":{"max_prefix_tokens":0,"max_prompt_tokens":3}}
    }))
    .unwrap();
    let original = repo.replace_revision(definition, None).await.unwrap();
    let designs = PgPrefixModelDesigns::new(rig.pool.clone(), NAMESPACE.into());
    (rig, repo, original, designs)
}

async fn capture(
    designs: &PgPrefixModelDesigns,
    provider: &ProviderDefinitionRevision,
    model: &str,
) -> PrefixModelSpecification {
    designs
        .capture(
            CapturePrefixModelRequest {
                provider_id: provider.definition.id.clone(),
                expected_provider_revision: provider.revision,
                model: model.into(),
            },
            vec![PrefixModelTarget {
                provider_id: provider.definition.id.clone(),
                provider_revision: provider.revision,
                model: model.into(),
                consumer: "configured".into(),
                configured_context_window: (model == provider.definition.default_model)
                    .then_some(provider.definition.context_window_tokens)
                    .flatten(),
            }],
        )
        .await
        .unwrap()
}

/// Insert a complete hash-consistent historical/corruption fixture through the
/// real schema. Existing immutable originals are neither changed nor deleted.
async fn retain_fixture(rig: &NativePostgres, value: &Value) -> String {
    let bytes = value.to_string();
    let id = sha(bytes.as_bytes());
    let revision = value["provider_revision"].as_i64().unwrap();
    rig.pool.get().await.unwrap().execute(
        "INSERT INTO reborn_prefix_model_specifications(specification_id,provider_namespace,provider_id,provider_revision,model,specification_bytes) VALUES($1,$2,$3,$4,$5,$6)",
        &[&id,&NAMESPACE,&value["provider_id"].as_str().unwrap(),&revision,&value["model"].as_str().unwrap(),&bytes],
    ).await.unwrap();
    id
}

#[tokio::test]
async fn model_snapshot_reads_original_policy_after_provider_cas_and_lost_response() {
    let (rig, repo, original, designs) = fixture().await;
    let first = capture(&designs, &original, MODEL).await;
    let mut edited = original.definition.clone();
    edited.context_window_tokens = Some(1000);
    edited
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prefix_tokens = 100;
    let replacement = repo
        .replace_revision(edited, Some(original.revision))
        .await
        .unwrap();
    let read = designs.get_specification(&first.id).await.unwrap();
    assert_eq!(read.specification_bytes, first.specification_bytes);
    let lost_response = designs
        .capture(
            CapturePrefixModelRequest {
                provider_id: original.definition.id.clone(),
                expected_provider_revision: original.revision,
                model: MODEL.into(),
            },
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(lost_response.id, first.id);
    assert_eq!(lost_response.specification_bytes, first.specification_bytes);
    let second = capture(&designs, &replacement, MODEL).await;
    assert_ne!(first.id, second.id);
    let before: Value = serde_json::from_str(&first.specification_bytes).unwrap();
    let after: Value = serde_json::from_str(&second.specification_bytes).unwrap();
    assert_eq!(before["model_token_limits"]["max_prefix_tokens"], 1);
    assert_eq!(after["model_token_limits"]["max_prefix_tokens"], 100);
    assert_eq!(before["configured_context_window"], 4);
    assert_eq!(after["configured_context_window"], 1000);
    for (specification, prefix, context) in [(&first, 1, 4), (&second, 100, 1000)] {
        let (retained, inputs) = designs
            .read_model_input(
                &specification.id,
                Some(PrefixBuildReservations {
                    requested_prefix_tokens: 3,
                    reserved_output_tokens: 1,
                    additional_framing_tokens: 0,
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            retained.specification_bytes,
            specification.specification_bytes
        );
        let inputs = inputs.unwrap();
        assert_eq!(inputs["max_prefix_tokens"], prefix);
        assert_eq!(inputs["max_prompt_tokens"], 2);
        assert_eq!(inputs["context_window_tokens"], context);
        assert_eq!(
            inputs["policy_ref"],
            format!("prefix-model-specification:{}", specification.id)
        );
    }
    let catalogue = designs.catalogue(vec![]).await.unwrap();
    assert_eq!(catalogue.specifications.len(), 2);
    assert!(!catalogue.design_generation_available);
    let foreign = PgPrefixModelDesigns::new(rig.pool.clone(), "different-owner".into());
    assert!(matches!(
        foreign.get_specification(&first.id).await,
        Err(Error::Conflict)
    ));
}

#[tokio::test]
async fn self_consistent_snapshot_hash_cannot_substitute_provider_facts() {
    let (rig, _, original, designs) = fixture().await;
    let captured = capture(&designs, &original, MODEL).await;
    let source: Value = serde_json::from_str(&captured.specification_bytes).unwrap();
    for (field, changed) in [
        ("provider_namespace", json!("different-owner")),
        ("protocol", json!("ollama")),
        ("configured_context_window", json!(262144)),
        ("context_evidence", json!("model-position-limit")),
        (
            "model_token_limits",
            json!({"max_prefix_tokens":100000,"max_prompt_tokens":14688}),
        ),
        ("model_token_limits_owner", json!("prefix-settings-copy")),
    ] {
        let mut forged = source.clone();
        forged[field] = changed;
        let id = retain_fixture(&rig, &forged).await;
        assert!(
            matches!(
                designs.get_specification(&id).await,
                Err(Error::Unavailable)
            ),
            "hash-consistent substituted {field} must fail closed"
        );
    }
    for missing in ["model_token_limits", "model_token_limits_owner"] {
        let mut partial = source.clone();
        partial.as_object_mut().unwrap().remove(missing);
        let id = retain_fixture(&rig, &partial).await;
        assert!(matches!(
            designs.get_specification(&id).await,
            Err(Error::Unavailable)
        ));
    }
    // No partial catalogue, fallback to old policy or newly accepted design.
    assert!(matches!(
        designs.catalogue(vec![]).await,
        Err(Error::Unavailable)
    ));
    assert_eq!(
        designs
            .get_specification(&captured.id)
            .await
            .unwrap()
            .specification_bytes,
        captured.specification_bytes
    );
}

#[tokio::test]
async fn historical_missing_limits_and_nondefault_context_remain_unknown() {
    let (rig, _, original, designs) = fixture().await;
    let captured = capture(&designs, &original, MODEL).await;
    let mut legacy: Value = serde_json::from_str(&captured.specification_bytes).unwrap();
    legacy.as_object_mut().unwrap().remove("model_token_limits");
    legacy
        .as_object_mut()
        .unwrap()
        .remove("model_token_limits_owner");
    let id = retain_fixture(&rig, &legacy).await;
    let historical = designs.get_specification(&id).await.unwrap();
    assert_eq!(historical.specification_bytes, legacy.to_string());
    let preserved: Value = serde_json::from_str(&historical.specification_bytes).unwrap();
    assert!(preserved.get("model_token_limits").is_none());
    let other = capture(&designs, &original, "other-model").await;
    let value: Value = serde_json::from_str(&other.specification_bytes).unwrap();
    assert!(value["configured_context_window"].is_null());
    assert_eq!(value["model_token_limits"]["max_prefix_tokens"], 0);
    assert_eq!(value["model_token_limits"]["max_prompt_tokens"], 3);
    for unavailable in [&id, &other.id] {
        assert!(matches!(
            designs
                .read_model_input(
                    unavailable,
                    Some(PrefixBuildReservations {
                        requested_prefix_tokens: 1,
                        reserved_output_tokens: 1,
                        additional_framing_tokens: 0,
                    }),
                )
                .await,
            Err(Error::InvalidRequest { .. })
        ));
    }
    assert!(matches!(
        designs
            .read_model_input(
                &captured.id,
                Some(PrefixBuildReservations {
                    requested_prefix_tokens: 1,
                    reserved_output_tokens: 0,
                    additional_framing_tokens: 0,
                }),
            )
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    let catalogue = designs.catalogue(vec![]).await.unwrap();
    assert_eq!(catalogue.specifications.len(), 3);
    assert!(!catalogue.design_generation_available);
}
