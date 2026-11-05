//! Actual live settings uptake + PG metadata. The large stored input below is
//! intentionally not admitted: this proves codec/custody, not large VM execution,
//! model quality, source validity or workflow activation.
use super::*;
use crate::runtime::RebornRuntime;
use brassclaw_product_workflow::{RebornServicesApi, WebUiAuthenticatedCaller};
use brassclaw_skills::value_contract::ContractLimits;
use uuid::Uuid;
fn triple(limits: ContractLimits) -> [usize; 3] {
    [limits.max_depth, limits.max_nodes, limits.max_bytes]
}
fn job_scope(scope: &TurnScope, id: Uuid) -> TurnScope {
    let mut scope = scope.clone();
    scope.thread_id = brassclaw_host_api::ThreadId::new(format!("prefix-job-{id}")).unwrap();
    scope
}
pub(super) async fn assert_original_input_limits(
    runtime: &Arc<RebornRuntime>,
    api: &dyn RebornServicesApi,
    caller: &WebUiAuthenticatedCaller,
) {
    use crate::prefix_job_definitions::{
        LEGACY_INPUT_LIMITS, Origin, Reservation, Store, Target, captured_input_limits,
    };
    let pool = runtime.installed_catalogue.pool.clone();
    let store = Store::new(pool.clone());
    let original = api
        .get_monty_vm_settings(caller.clone())
        .await
        .unwrap()
        .settings;
    let program = runtime
        .installed_catalogue
        ._compiler_program
        .as_ref()
        .unwrap();
    let revision: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let target = Target::Profile {
        name: "base-prompt".into(),
        revision: u64::try_from(revision).unwrap(),
    };
    let request =
        json!({"profile":"base-prompt","expected_revision":revision,"mode":"source_only"});
    let id = Uuid::new_v4();
    let scope = TurnScope::new_with_owner(
        runtime.thread_scope.tenant_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
        brassclaw_host_api::ThreadId::new(format!("prefix-job-{id}")).unwrap(),
        Some(runtime.actor_user_id.clone()),
    );
    let thread = brassclaw_threads::ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id: runtime.thread_scope.agent_id.clone(),
        project_id: scope.project_id.clone(),
        owner_user_id: Some(runtime.actor_user_id.clone()),
    };
    let input = json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":"x".repeat(LEGACY_INPUT_LIMITS.max_bytes)});
    let encoded_len = serde_json::to_string(&input).unwrap().len();
    assert!(encoded_len > LEGACY_INPUT_LIMITS.max_bytes);
    let mut raised_policy = original.execution_limits;
    raised_policy.max_value_bytes = u64::try_from(encoded_len).unwrap().checked_add(1).unwrap();
    raised_policy.max_ipc_frame_bytes = raised_policy.max_ipc_frame_bytes.max(
        raised_policy
            .max_value_bytes
            .checked_add(brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES)
            .unwrap(),
    );
    raised_policy.validate().unwrap();
    let raised = api
        .update_monty_vm_settings(
            caller.clone(),
            serde_json::from_value(
                json!({"expected_revision":original.revision,"execution_limits":raised_policy}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let effective = runtime
        .installed_catalogue
        .preparation_policy()
        .await
        .unwrap();
    assert_eq!(
        effective, raised_policy,
        "capture the actual effective view, not merely desired settings"
    );
    let limits = captured_input_limits(effective).unwrap();
    let reservation = Reservation {
        id,
        target: &target,
        operation: "generate",
        mode: "source_only",
        request: &request,
        program,
        input: &input,
        original_input_limits: limits,
        origin: &Origin::Chat {
            scope: scope.clone(),
            thread_scope: thread.clone(),
        },
        created_by: runtime.actor_user_id.as_str(),
    };
    let stored = store.reserve(Reservation { ..reservation }).await.unwrap();
    assert_eq!(stored.input, input);
    assert_eq!(triple(stored.original_input_limits), triple(limits));
    assert!(!pool.get().await.unwrap().query_one("SELECT original_input_limits IS NULL FROM brassclaw_prefix_job_definitions WHERE job_id=$1",&[&id]).await.unwrap().get::<_,bool>(0));
    drop(stored);
    // Lower only live task-data capacity, leaving original bytes/limits intact.
    let mut reduced_policy = raised_policy;
    reduced_policy.max_value_bytes = u64::try_from(LEGACY_INPUT_LIMITS.max_bytes / 2).unwrap();
    let reduced=api.update_monty_vm_settings(caller.clone(),serde_json::from_value(json!({"expected_revision":raised.settings.revision,"execution_limits":reduced_policy})).unwrap()).await.unwrap();
    let current = runtime
        .installed_catalogue
        .preparation_policy()
        .await
        .unwrap();
    assert_eq!(current, reduced_policy);
    let reduced_limits = captured_input_limits(current).unwrap();
    let reread = store.get(id).await.unwrap().unwrap();
    assert_eq!(reread.input, input);
    assert_eq!(triple(reread.original_input_limits), triple(limits));
    drop(reread);
    let retried = store
        .reserve(Reservation {
            original_input_limits: reduced_limits,
            ..reservation
        })
        .await
        .unwrap();
    assert_eq!(triple(retried.original_input_limits), triple(limits));
    drop(retried);
    let too_large_id = Uuid::new_v4();
    let too_large_scope = job_scope(&scope, too_large_id);
    assert!(
        store
            .reserve(Reservation {
                id: too_large_id,
                origin: &Origin::Chat {
                    scope: too_large_scope.clone(),
                    thread_scope: thread.clone()
                },
                original_input_limits: reduced_limits,
                ..reservation
            })
            .await
            .is_err(),
        "a new input still obeys current captured limits"
    );
    let small = json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":"small metadata-only fixture"});
    let small_id = Uuid::new_v4();
    let small_scope = job_scope(&scope, small_id);
    let small_reservation = Reservation {
        id: small_id,
        origin: &Origin::Chat {
            scope: small_scope.clone(),
            thread_scope: thread.clone(),
        },
        input: &small,
        original_input_limits: reduced_limits,
        ..reservation
    };
    store
        .reserve(Reservation {
            ..small_reservation
        })
        .await
        .unwrap();
    for invalid in [
        ContractLimits {
            max_depth: 0,
            ..reduced_limits
        },
        ContractLimits {
            max_nodes: 0,
            ..reduced_limits
        },
        ContractLimits {
            max_bytes: 0,
            ..reduced_limits
        },
    ] {
        let invalid_id = Uuid::new_v4();
        let invalid_scope = job_scope(&scope, invalid_id);
        assert!(
            store
                .reserve(Reservation {
                    id: invalid_id,
                    origin: &Origin::Chat {
                        scope: invalid_scope.clone(),
                        thread_scope: thread.clone()
                    },
                    original_input_limits: invalid,
                    ..small_reservation
                })
                .await
                .is_err()
        );
    }
    let legacy = Uuid::new_v4();
    let legacy_scope = serde_json::to_value(job_scope(&scope, legacy)).unwrap();
    let client = pool.get().await.unwrap();
    // Legacy import shape: column is absent/NULL, existing byte/hash columns
    // remain literal originals. This test does not rewrite any historical row.
    client.execute("INSERT INTO brassclaw_prefix_job_definitions(job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by) SELECT $1,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,$3::jsonb,thread_scope,created_by FROM brassclaw_prefix_job_definitions WHERE job_id=$2",&[&legacy,&small_id,&legacy_scope]).await.unwrap();
    let saved = store.get(legacy).await.unwrap().unwrap();
    assert_eq!(saved.input, small);
    assert_eq!(
        triple(saved.original_input_limits),
        triple(LEGACY_INPUT_LIMITS)
    );
    assert!(client.query_one("SELECT original_input_limits IS NULL FROM brassclaw_prefix_job_definitions WHERE job_id=$1",&[&legacy]).await.unwrap().get::<_,bool>(0));
    let valid = json!({"max_depth":reduced_limits.max_depth,"max_nodes":reduced_limits.max_nodes,"max_bytes":reduced_limits.max_bytes});
    let mut invalid = Vec::new();
    for field in ["max_depth", "max_nodes", "max_bytes"] {
        for value in [Value::Null, json!(0), json!(-1), json!(1.5), json!(true)] {
            let mut bad = valid.clone();
            bad[field] = value;
            invalid.push(bad);
        }
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        invalid.push(missing);
    }
    let mut extra = valid.clone();
    extra["unexpected"] = json!(1);
    invalid.push(extra);
    invalid.extend([Value::Null, json!([])]);
    for limits in invalid {
        let invalid_id = Uuid::new_v4();
        let invalid_scope = serde_json::to_value(job_scope(&scope, invalid_id)).unwrap();
        assert!(client.execute("INSERT INTO brassclaw_prefix_job_definitions(job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by,original_input_limits) SELECT $1,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,$4::jsonb,thread_scope,created_by,$3 FROM brassclaw_prefix_job_definitions WHERE job_id=$2",&[&invalid_id,&small_id,&limits,&invalid_scope]).await.is_err(),"SQL must reject malformed/NULL-valued bounds as well");
    }
    assert!(client.execute("UPDATE brassclaw_prefix_job_definitions SET original_input_limits=$2 WHERE job_id=$1",&[&legacy,&valid]).await.is_err(),"legacy originals remain immutable");
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=ANY($1)",
                &[&vec![id, small_id, legacy]]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    drop(client);
    let restored=api.update_monty_vm_settings(caller.clone(),serde_json::from_value(json!({"expected_revision":reduced.settings.revision,"execution_limits":original.execution_limits})).unwrap()).await.unwrap();
    assert_eq!(
        restored.settings.execution_limits,
        original.execution_limits
    );
    assert_eq!(
        runtime
            .installed_catalogue
            .preparation_policy()
            .await
            .unwrap(),
        original.execution_limits
    );
}
