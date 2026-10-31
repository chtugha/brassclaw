//! Actual PostgreSQL model-owner persistence; no inference or design acceptance.
use super::*;
use crate::{pg_provider_repo::PgProviderRepo, runtime::test_pg::native_pg::NativePostgres};
use brassclaw_product_workflow::{CapturePrefixModelRequest, PrefixModelTarget};
use serde_json::json;

async fn fixture() -> (
    NativePostgres,
    PgPrefixModelDesigns,
    PgProviderRepo,
    PrefixModelSpecification,
) {
    let rig = NativePostgres::start().await;
    let repo = PgProviderRepo::new(rig.pool.as_ref().clone(), "setup-draft-owner");
    repo.initialize().await.unwrap();
    let definition = serde_json::from_value(json!({
        "id":"draft-target", "protocol":"open_ai_completions", "default_base_url":"http://127.0.0.1:1/v1",
        "api_key_required":false, "model_env":"UNUSED_SETUP_DRAFT_FIXTURE", "default_model":"original-model",
        "description":"Original unaccepted setup target", "context_window_tokens":4,
        "model_token_limits":{"original-model":{"max_prefix_tokens":1,"max_prompt_tokens":2}}
    })).unwrap();
    let provider = repo.replace_revision(definition, None).await.unwrap();
    let designs = PgPrefixModelDesigns::new(rig.pool.clone(), "setup-draft-owner".into());
    let specification = designs
        .capture(
            CapturePrefixModelRequest {
                provider_id: provider.definition.id.clone(),
                expected_provider_revision: provider.revision,
                model: "original-model".into(),
            },
            vec![PrefixModelTarget {
                provider_id: provider.definition.id,
                provider_revision: provider.revision,
                model: "original-model".into(),
                consumer: "configured".into(),
                configured_context_window: Some(4),
            }],
        )
        .await
        .unwrap();
    (rig, designs, repo, specification)
}
fn request(expected: u64, edits: &[(Field, Option<u64>)]) -> UpdatePrefixModelSetupDraftRequest {
    UpdatePrefixModelSetupDraftRequest {
        operation_id: uuid::Uuid::new_v4(),
        expected_revision: expected,
        edits: edits
            .iter()
            .map(|(field, value)| PrefixModelSetupEdit {
                field: *field,
                value: *value,
            })
            .collect(),
    }
}

#[test]
fn setup_edit_contract_keeps_explicit_null_and_rejects_missing_duplicates_and_false_numbers() {
    let clear: UpdatePrefixModelSetupDraftRequest = serde_json::from_str(
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":null}]}"#,
    )
    .unwrap();
    assert!(clear.valid());
    assert!(clear.edits[0].value.is_none());
    assert!(serde_json::from_str::<PrefixModelSetupValue>(r#"{"origin":"manual"}"#).is_err());
    assert!(
        serde_json::from_str::<PrefixModelSetupValue>(r#"{"origin":"manual","value":null}"#)
            .is_ok()
    );
    assert!(
        serde_json::from_str::<UpdatePrefixModelSetupDraftRequest>(
            r#"{"expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":1}]}"#
        )
        .is_err()
    );
    let mut nil = clear.clone();
    nil.operation_id = uuid::Uuid::nil();
    assert!(!nil.valid());
    for value in [
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens"}]}"#,
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":true}]}"#,
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":1.0}]}"#,
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":-1}]}"#,
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":1,"origin":"provider_configuration"}]}"#,
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"max_prefix_tokens","field":"max_prompt_tokens","value":1}]}"#,
        r#"{"operation_id":"00000000-0000-0000-0000-000000000001","expected_revision":0,"edits":[{"field":"unknown","value":1}]}"#,
    ] {
        assert!(
            serde_json::from_str::<UpdatePrefixModelSetupDraftRequest>(value).is_err(),
            "{value}"
        );
    }
    assert!(!request(0, &[]).valid());
    assert!(
        !request(
            0,
            &[
                (Field::MaxPrefixTokens, Some(0)),
                (Field::MaxPrefixTokens, None)
            ]
        )
        .valid()
    );
    assert!(!request(0, &[(Field::MaxPromptTokens, Some(0))]).valid());
    assert!(!request(0, &[(Field::ReservedOutputTokens, Some(0))]).valid());
    assert!(
        !request(
            0,
            &[(
                Field::MaxPrefixTokens,
                Some(PREFIX_SETUP_MAX_EXACT_INTEGER + 1)
            )]
        )
        .valid()
    );
    assert!(
        !request(
            PREFIX_SETUP_MAX_EXACT_INTEGER,
            &[(Field::MaxPrefixTokens, None)]
        )
        .valid()
    );
    assert!(
        request(
            0,
            &[
                (Field::MaxPrefixTokens, Some(0)),
                (Field::AdditionalFramingTokens, Some(0))
            ]
        )
        .valid()
    );
}

#[tokio::test]
async fn setup_draft_is_original_scoped_manual_history_not_provider_policy_or_design_approval() {
    let (rig, designs, repo, spec) = fixture().await;
    let basis = designs.setup_draft(&spec.id, None).await.unwrap();
    assert_eq!(basis.revision, 0);
    assert!(
        basis.fields[&Field::MaxPrefixTokens]
            == PrefixModelSetupValue {
                value: Some(1),
                origin: Origin::ProviderConfiguration
            }
    );
    assert!(
        basis.fields[&Field::ReservedOutputTokens]
            == PrefixModelSetupValue {
                value: None,
                origin: Origin::Unset
            }
    );
    let saved = designs
        .update_setup_draft(
            &spec.id,
            request(
                0,
                &[
                    (Field::MaxPrefixTokens, Some(0)),
                    (Field::ReservedOutputTokens, None),
                ],
            ),
        )
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    assert!(
        saved.fields[&Field::MaxPrefixTokens]
            == PrefixModelSetupValue {
                value: Some(0),
                origin: Origin::Manual
            }
    );
    assert!(
        saved.fields[&Field::ReservedOutputTokens]
            == PrefixModelSetupValue {
                value: None,
                origin: Origin::Manual
            },
        "explicit manual clear remains protected despite matching the old null value"
    );
    assert!(saved.fields[&Field::MaxPromptTokens] == basis.fields[&Field::MaxPromptTokens]);
    let current = repo
        .get_revision("draft-target", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.revision, 1);
    assert_eq!(
        serde_json::to_value(&current.definition).unwrap()["model_token_limits"]["original-model"]
            ["max_prefix_tokens"],
        1
    );
    let mut changed = current.definition;
    changed
        .model_token_limits
        .0
        .get_mut("original-model")
        .unwrap()
        .max_prompt_tokens = 3;
    repo.replace_revision(changed, Some(1)).await.unwrap();
    assert!(
        designs.setup_draft(&spec.id, None).await.unwrap() == saved,
        "later canonical changes never refill the old setup snapshot"
    );
    assert_eq!(
        designs
            .get_specification(&spec.id)
            .await
            .unwrap()
            .specification_bytes,
        spec.specification_bytes
    );
    let foreign = PgPrefixModelDesigns::new(rig.pool.clone(), "another-owner".into());
    assert!(foreign.setup_draft(&spec.id, None).await.is_err());
    assert!(
        foreign
            .update_setup_draft(&spec.id, request(1, &[(Field::MaxPrefixTokens, Some(1))]))
            .await
            .is_err()
    );
    let client = rig.pool.get().await.unwrap();
    let count: i64 = client
        .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    let count: i64 = client
        .query_one(
            "SELECT count(*) FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1",
            &[&spec.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    assert!(client.execute("UPDATE reborn_prefix_model_setup_drafts SET draft_bytes=draft_bytes WHERE specification_id=$1",&[&spec.id]).await.is_err());
    assert!(
        client
            .execute(
                "DELETE FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1",
                &[&spec.id]
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn setup_draft_cas_fences_whole_snapshot_and_retries_original_revision_after_later_edits() {
    let (rig, designs, _repo, spec) = fixture().await;
    let first = request(
        0,
        &[
            (Field::MaxPrefixTokens, Some(0)),
            (Field::ReservedOutputTokens, Some(1)),
        ],
    );
    let saved = designs
        .update_setup_draft(&spec.id, first.clone())
        .await
        .unwrap();
    let later = designs
        .update_setup_draft(&spec.id, request(1, &[(Field::MaxPromptTokens, None)]))
        .await
        .unwrap();
    assert_eq!(later.revision, 2);
    let mut retry_request = request(
        0,
        &[
            (Field::ReservedOutputTokens, Some(1)),
            (Field::MaxPrefixTokens, Some(0)),
        ],
    );
    retry_request.operation_id = first.operation_id;
    let retry = designs
        .update_setup_draft(&spec.id, retry_request)
        .await
        .unwrap();
    // A separate stale action cannot claim success from a coincidentally equal
    // older edit. Reusing an operation with altered payload/base also conflicts.
    assert!(matches!(
        designs
            .update_setup_draft(
                &spec.id,
                request(
                    0,
                    &[
                        (Field::MaxPrefixTokens, Some(0)),
                        (Field::ReservedOutputTokens, Some(1))
                    ]
                )
            )
            .await,
        Err(Error::Conflict)
    ));
    let mut wrong = first.clone();
    wrong.expected_revision = 1;
    assert!(matches!(
        designs.update_setup_draft(&spec.id, wrong).await,
        Err(Error::Conflict)
    ));
    let mut wrong = first.clone();
    wrong.edits[0].value = Some(1);
    assert!(matches!(
        designs.update_setup_draft(&spec.id, wrong).await,
        Err(Error::Conflict)
    ));
    assert!(
        retry == saved,
        "lost-ack readback returns the exact old result, never replays over a later edit"
    );
    assert!(designs.setup_draft(&spec.id, None).await.unwrap() == later);
    assert!(designs.setup_draft(&spec.id, Some(1)).await.unwrap() == saved);
    assert_eq!(
        designs
            .setup_draft(&spec.id, Some(0))
            .await
            .unwrap()
            .revision,
        0
    );
    assert!(matches!(
        designs.setup_draft(&spec.id, Some(9)).await,
        Err(Error::Conflict)
    ));
    assert!(
        designs
            .setup_draft(&spec.id, Some(PREFIX_SETUP_MAX_EXACT_INTEGER + 1))
            .await
            .is_err()
    );
    assert!(matches!(
        designs
            .update_setup_draft(
                &spec.id,
                request(0, &[(Field::AdditionalFramingTokens, Some(0))])
            )
            .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        designs
            .update_setup_draft(
                &spec.id,
                request(5, &[(Field::AdditionalFramingTokens, Some(0))])
            )
            .await,
        Err(Error::Conflict)
    ));
    let a = PgPrefixModelDesigns::new(rig.pool.clone(), "setup-draft-owner".into());
    let b = PgPrefixModelDesigns::new(rig.pool.clone(), "setup-draft-owner".into());
    let (one, two) = tokio::join!(
        a.update_setup_draft(
            &spec.id,
            request(2, &[(Field::AdditionalFramingTokens, Some(0))])
        ),
        b.update_setup_draft(&spec.id, request(2, &[(Field::MaxPrefixTokens, Some(1))]))
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        usize::from(matches!(one, Err(Error::Conflict)))
            + usize::from(matches!(two, Err(Error::Conflict))),
        1
    );
    assert_eq!(
        designs.setup_draft(&spec.id, None).await.unwrap().revision,
        3
    );
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1",
            &[&spec.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 3);
}

#[tokio::test]
async fn setup_draft_reader_rejects_self_consistent_but_false_configuration_origin_without_fallback()
 {
    let (rig, designs, _repo, spec) = fixture().await;
    let saved = designs
        .update_setup_draft(&spec.id, request(0, &[(Field::MaxPrefixTokens, Some(0))]))
        .await
        .unwrap();
    let mut forged = saved.clone();
    forged.revision = 2;
    forged.operation_id = Some(uuid::Uuid::new_v4());
    let forged_operation = forged.operation_id.unwrap();
    forged.fields.insert(
        Field::MaxPromptTokens,
        PrefixModelSetupValue {
            value: Some(3),
            origin: Origin::ProviderConfiguration,
        },
    );
    let edits = vec![PrefixModelSetupEdit {
        field: Field::MaxPrefixTokens,
        value: Some(0),
    }];
    let stored = StoredDraft {
        format: "brassclaw-prefix-model-setup-draft/1".into(),
        base_revision: 1,
        edits,
        draft: forged,
    };
    let bytes =
        crate::prefix_generations::canonical(&serde_json::to_value(stored).unwrap()).unwrap();
    rig.pool.get().await.unwrap().execute("INSERT INTO reborn_prefix_model_setup_drafts(specification_id,revision,base_revision,draft_bytes,draft_checksum,operation_id) VALUES($1,2,1,$2,$3,$4)",&[&spec.id,&bytes,&sha(bytes.as_bytes()),&forged_operation]).await.unwrap();
    assert!(
        matches!(
            designs.setup_draft(&spec.id, None).await,
            Err(Error::Unavailable)
        ),
        "hash consistency does not prove configuration origin"
    );
    assert!(
        designs
            .update_setup_draft(&spec.id, request(2, &[(Field::MaxPrefixTokens, Some(1))]))
            .await
            .is_err()
    );
    let mut missing = serde_json::to_value(StoredDraft {
        format: "brassclaw-prefix-model-setup-draft/1".into(),
        base_revision: 2,
        edits: vec![PrefixModelSetupEdit {
            field: Field::MaxPrefixTokens,
            value: Some(0),
        }],
        draft: saved.clone(),
    })
    .unwrap();
    let missing_operation = uuid::Uuid::new_v4();
    missing["draft"]["revision"] = json!(3);
    missing["draft"]["operation_id"] = json!(missing_operation);
    missing["draft"]["fields"]["reserved_output_tokens"]
        .as_object_mut()
        .unwrap()
        .remove("value");
    let bytes = crate::prefix_generations::canonical(&missing).unwrap();
    rig.pool.get().await.unwrap().execute("INSERT INTO reborn_prefix_model_setup_drafts(specification_id,revision,base_revision,draft_bytes,draft_checksum,operation_id) VALUES($1,3,2,$2,$3,$4)",&[&spec.id,&bytes,&sha(bytes.as_bytes()),&missing_operation]).await.unwrap();
    assert!(
        matches!(
            designs.setup_draft(&spec.id, Some(3)).await,
            Err(Error::Unavailable)
        ),
        "missing output value cannot default to null despite a consistent hash"
    );
    let mut retry = request(0, &[(Field::MaxPrefixTokens, Some(0))]);
    retry.operation_id = saved.operation_id.unwrap();
    assert!(
        designs.update_setup_draft(&spec.id, retry).await.unwrap() == saved,
        "original operation remains readable without consulting the corrupted head"
    );
}

#[tokio::test]
async fn setup_product_service_uses_same_owner_without_model_or_orchestrator_requirement() {
    use brassclaw_host_api::{TenantId, UserId};
    use brassclaw_product_workflow::{InterceptorConfigService, WebUiAuthenticatedCaller};
    let (rig, designs, _repo, spec) = fixture().await;
    let service = crate::interceptor_config_service::RebornInterceptorConfigService::new(
        rig.pool.clone(),
        "setup-draft-owner",
    );
    let caller = WebUiAuthenticatedCaller::new(
        TenantId::new("setup-draft-owner").unwrap(),
        UserId::new("setup-operator").unwrap(),
        None,
        None,
    );
    let initial = service
        .model_setup_draft(caller.clone(), &spec.id, None)
        .await
        .unwrap();
    assert_eq!(initial.revision, 0);
    let updated = service
        .update_model_setup_draft(
            caller.clone(),
            &spec.id,
            request(0, &[(Field::MaxPrefixTokens, Some(0))]),
        )
        .await
        .unwrap();
    assert_eq!(updated.revision, 1);
    assert!(designs.setup_draft(&spec.id, None).await.unwrap() == updated);
    assert_eq!(
        service
            .model_setup_draft(caller.clone(), &spec.id, Some(0))
            .await
            .unwrap()
            .revision,
        0
    );
    assert!(
        service
            .model_setup_draft(caller, &spec.id, None)
            .await
            .unwrap()
            == updated
    );
}
