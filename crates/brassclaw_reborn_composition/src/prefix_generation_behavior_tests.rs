//! Real candidate workflow checks. Development evidence is not a system_seed
//! producer receipt and does not approve, activate or advertise these usages.
use crate::{bootstrap_validation, installed_monty_catalogue::InstalledMontyCatalogue};
use brassclaw_engine::executor::retained_recipe::{RetainedProgram, RetainedRecipeExecution};
use brassclaw_monty_host::{
    global::GlobalBounds,
    heap::HeapSettings,
    process::{PortAnswer, RootBoot, TaskSettings},
    transport_actor::ActorLimits,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};

pub(super) async fn assert_native_generation_usages(catalogue: &Arc<InstalledMontyCatalogue>) {
    let limits = catalogue._generation_candidates[1].preparation_limits();
    let package = crate::prefix_installation_baseline::assemble(
        &catalogue.worker,
        limits,
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert_native_package(catalogue, package, false).await;
}

pub(super) async fn assert_native_large_generation(
    catalogue: &Arc<InstalledMontyCatalogue>,
    package: String,
) {
    assert!(package.len() > brassclaw_skills::component_revision::REVISION_LIMITS.max_bytes);
    assert_native_package(catalogue, package, true).await;
}

async fn assert_native_package(
    catalogue: &Arc<InstalledMontyCatalogue>,
    package: String,
    positive_only: bool,
) {
    let limits = catalogue._generation_candidates[1].preparation_limits();
    let generation = hex::encode(Sha256::digest(package.as_bytes()));
    // Utility workers have a separate physical ceiling from the instance's
    // shared VM. Derive the bounded case frame from its complete encoded data;
    // retain transport/adapter headroom before assigning any VM heap ceiling.
    let adapter = brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize;
    let frame = serde_json::to_vec(&json!(package))
        .unwrap()
        .len()
        .checked_add(1024 * 1024)
        .unwrap();
    assert!(frame <= limits.process.max_frame_bytes);
    let mut process = limits.process;
    process.max_frame_bytes = frame;
    let mut case_bounds = limits.bounds;
    case_bounds.max_value_bytes = frame;
    assert!(case_bounds.max_source_bytes <= frame && case_bounds.max_stdout_bytes <= frame);
    let reserve = frame.checked_mul(2).unwrap().checked_add(adapter).unwrap();
    let heap = process.hard_memory_bytes.checked_sub(reserve).unwrap();
    assert!(
        heap > frame,
        "the complete baseline must fit the configured physical validation ceiling"
    );

    let client = catalogue.pool.get().await.unwrap();
    let revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let existed: bool = client.query_one("SELECT EXISTS(SELECT 1 FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1)", &[&generation]).await.unwrap().get(0);
    let before: i64 = client
        .query_one("SELECT count(*) FROM reborn_prefix_generations", &[])
        .await
        .unwrap()
        .get(0);
    let active: uuid::Uuid = client
        .query_one(
            "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let serving: Value = client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text), '[]'::jsonb) FROM reborn_basic_prompt_store s", &[]).await.unwrap().get(0);
    drop(client);
    let mut first = Value::Null;
    for (retain, task_input, denied, success, expected_calls) in [
        (
            true,
            json!({"profile":"base-prompt","revision":revision.to_string(),"generation_bytes":package}),
            false,
            true,
            1,
        ),
        (
            true,
            json!({"profile":"base-prompt","revision":revision.to_string(),"generation_bytes":package}),
            false,
            true,
            1,
        ),
        (
            false,
            json!({"profile":"base-prompt","generation":generation}),
            false,
            true,
            1,
        ),
        (
            true,
            json!({"profile":"base-prompt","revision":(revision+1).to_string(),"generation_bytes":package}),
            false,
            false,
            1,
        ),
        (
            true,
            json!({"profile":"base-prompt","revision":revision.to_string(),"generation_bytes":"{not-a-package}"}),
            false,
            false,
            1,
        ),
        (
            true,
            json!({"profile":"base-prompt","revision":"01","generation_bytes":package}),
            false,
            false,
            0,
        ),
        (
            true,
            json!({"profile":"base-prompt","revision":revision.to_string(),"generation_bytes":package}),
            true,
            false,
            1,
        ),
        (
            false,
            json!({"profile":"base-prompt","generation":"not-a-hash"}),
            false,
            false,
            1,
        ),
        (
            false,
            json!({"profile":"base-prompt","generation":"a".repeat(64)}),
            false,
            false,
            1,
        ),
        (
            false,
            json!({"profile":"base-prompt","generation":generation}),
            true,
            false,
            1,
        ),
    ].into_iter().take(if positive_only { 3 } else { 10 }) {
        let inspected = catalogue._generation_candidates[usize::from(retain)].clone();
        let RetainedProgram::Tools(program) = inspected.program() else {
            panic!("actual Tool program required")
        };
        let bound = program.inputs().bind_task_inputs(&task_input).unwrap();
        let admitted = Box::pin(bootstrap_validation::admit(
            catalogue.pool.clone(),
            "bootstrap-prefix-generation",
            "Contained candidate storage validation",
        ))
        .await
        .unwrap();
        admitted
            .admission
            .retain_recipe_selection(program.inputs().instruction())
            .await
            .unwrap();
        let tools = crate::installed_monty_catalogue::bootstrap_generation_tools(
            catalogue.pool.clone(),
            catalogue.kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            denied,
        )
        .unwrap();
        let boot = RootBoot {
            source: crate::bootstrap_reply_validation::ROOT.into(),
            checksum: Sha256::digest(crate::bootstrap_reply_validation::ROOT.as_bytes()).into(),
            aliases: ["await_next_task", "enter_task", "validation_hold"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            bounds: GlobalBounds {
                values: case_bounds,
                workers: 1,
                max_pending_calls: 8,
            },
            heap_settings: Some(HeapSettings {
                revision: 1,
                max_vm_bytes: heap,
            }),
            adapter_reserve_bytes: brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize,
            startup_timeout: Duration::from_secs(10),
            task_settings: TaskSettings {
                revision: 1,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            max_recipe_contexts: 8,
        };
        let actor = ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: frame * 2,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: frame * 2,
        };
        let (mut owner, transport, _, task) = bootstrap_validation::start(
            &catalogue.worker,
            boot,
            process,
            actor,
            admitted.input.clone(),
        )
        .await
        .unwrap();
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected.clone())
                .unwrap();
        let result = if retain {
            let preparation = program
                .inputs()
                .bind_step_inputs("0:1", &json!({"revision":bound["revision"]}))
                .unwrap();
            match execution
                .run_step(&transport, "0:1", &preparation, None)
                .await
            {
                Ok(revision) => {
                    let input = program.inputs().bind_step_inputs("0:3", &json!({"profile":bound["profile"],"expected_profile_revision":revision,"generation_bytes":bound["generation_bytes"]})).unwrap();
                    execution
                        .run_step(&transport, "0:3", &input, Some(tools.as_ref()))
                        .await
                }
                Err(error) => Err(error),
            }
        } else {
            let input = program.inputs().bind_step_inputs("0:2", &bound).unwrap();
            execution
                .run_step(&transport, "0:2", &input, Some(tools.as_ref()))
                .await
        };
        assert_eq!(
            result.is_ok(),
            success,
            "retain={retain} denied={denied} result={result:?}"
        );
        assert_eq!(execution.host_answers().len(), expected_calls);
        for observation in execution.observations().values() {
            assert!(observation.settled());
            assert!(observation.observation_error().is_none());
        }
        if success {
            let value = result.unwrap();
            assert!(execution.is_complete());
            assert!(execution.transport_failure().is_none());
            if retain {
                assert_eq!(value["generation"], generation);
                assert_eq!(value["qualification"], "quarantined");
                if first.is_null() {
                    first = value;
                } else {
                    assert_eq!(
                        value, first,
                        "retry must keep the original receipt and timestamp"
                    );
                }
                assert_eq!(execution.observation_order().unwrap(), &["0:1", "0:3"]);
            } else {
                assert_eq!(value["generation_bytes"], package);
                assert_eq!(value["summary"], first);
                assert_eq!(execution.observation_order().unwrap(), &["0:2"]);
            }
        } else {
            assert!(!execution.is_complete());
            assert!(execution.failed_step_id().is_some());
            if expected_calls == 1 {
                assert!(matches!(
                    execution.host_answers()[0].1,
                    PortAnswer::TerminalError { .. }
                ));
            }
        }
        assert!(
            admitted.host.finalized_reply_ref().is_none(),
            "storage cannot publish or activate a reply"
        );
        bootstrap_validation::close(&mut owner, &admitted)
            .await
            .unwrap();
        let client = catalogue.pool.get().await.unwrap();
        let selected: String = client.query_one(
            "SELECT selection_bytes FROM brassclaw_monty_recipe_selections WHERE run_id=$1 AND recipe_id=$2",
            &[&admitted.attempt.run_id.as_uuid(), &program.inputs().instruction().recipe().uuid],
        ).await.unwrap().get(0);
        assert_eq!(
            selected,
            program
                .inputs()
                .instruction()
                .retained_selection()
                .unwrap()
                .exact_bytes()
        );
        let rows = client.query("SELECT phase,attempt_count,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&admitted.attempt.run_id.as_uuid()]).await.unwrap();
        assert_eq!(rows.len(), expected_calls);
        if positive_only && !retain {
            let encoded: i32 = client.query_one("SELECT octet_length(answer_bytes) FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&admitted.attempt.run_id.as_uuid()]).await.unwrap().get(0);
            assert!(encoded as usize > brassclaw_skills::component_revision::REVISION_LIMITS.max_bytes,
                "the actual large Read answer must be durably retained, not replaced by a summary");
        }
        for row in rows {
            assert_eq!(row.get::<_, String>(0), "answered");
            assert_eq!(row.get::<_, i16>(1), 1);
            assert!(row.get::<_, Option<String>>(2).is_some());
        }
        let count: i64 = client
            .query_one("SELECT count(*) FROM reborn_prefix_generations", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            count,
            before + i64::from(!existed),
            "negative cases and retries must not create another row"
        );
    }
    let client = catalogue.pool.get().await.unwrap();
    let actual: String = client.query_one("SELECT generation_bytes FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1", &[&generation]).await.unwrap().get(0);
    assert_eq!(actual, package);
    assert_eq!(
        client
            .query_one(
                "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton",
                &[]
            )
            .await
            .unwrap()
            .get::<_, uuid::Uuid>(0),
        active
    );
    assert_eq!(client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text), '[]'::jsonb) FROM reborn_basic_prompt_store s", &[]).await.unwrap().get::<_, Value>(0), serving);
}

/// Development-only full compiler execution: actual preloaded PythonCode,
/// registered hash/storage dispatch, durable intents and immutable SQL readback.
/// This intentionally creates no approval or activated catalogue entry.
pub(super) async fn assert_native_source_compiler(catalogue: &Arc<InstalledMontyCatalogue>) {
    // Exercise the exact installation-owned graph, rather than retaining a
    // second set of fixture components under unrelated UUIDs.
    let inspected = catalogue._source_compiler_candidates.source.clone();
    let RetainedProgram::Tools(program) = inspected.program() else {
        panic!("Tool program required")
    };
    let client = catalogue.pool.get().await.unwrap();
    let revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let serving:Value=client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb) FROM reborn_basic_prompt_store s",&[]).await.unwrap().get(0);
    drop(client);
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let oracle=std::process::Command::new("python3").args(["-c","import json; from test_prefix_compile_logic import native_fixture; print(json.dumps(native_fixture('base-prompt','literal-reference/v2-deduplicated'),ensure_ascii=False))"]).env("PYTHONPATH",checkout.join("scripts/prefix")).output().unwrap();
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let cases: Vec<Value> = serde_json::from_slice(&oracle.stdout).unwrap();
    let package = cases[4]["expected"].as_str().unwrap();
    let generation = hex::encode(Sha256::digest(package.as_bytes()));
    let limits = inspected.preparation_limits();
    // A frame carries one stage exchange, never the concatenation of every
    // independent CPython oracle input and output retained by this test.
    let frame = cases
        .iter()
        .flat_map(|case| [&case["inputs"], &case["expected"]])
        .map(|value| serde_json::to_vec(value).unwrap().len())
        .max()
        .unwrap()
        .checked_mul(2)
        .unwrap()
        .checked_add(1024 * 1024)
        .unwrap();
    assert!(frame <= limits.process.max_frame_bytes);
    let mut process = limits.process;
    process.max_frame_bytes = frame;
    let mut bounds = limits.bounds;
    bounds.max_value_bytes = frame;
    let adapter = brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize;
    let heap = process
        .hard_memory_bytes
        .checked_sub(frame * 2 + adapter)
        .unwrap();
    assert!(heap > frame);
    for mode in ["valid", "duplicate", "missing-guide", "denied"] {
        let mut source: Value =
            serde_json::from_str(cases[0]["inputs"]["snapshot_bytes"].as_str().unwrap()).unwrap();
        if mode == "missing-guide" {
            source["sources"].as_array_mut().unwrap().remove(0);
        }
        let task_inputs = json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":crate::prefix_generations::canonical(&source).unwrap()});
        program.inputs().bind_task_inputs(&task_inputs).unwrap();
        let admitted = Box::pin(bootstrap_validation::admit(
            catalogue.pool.clone(),
            "prefix-compiler-development",
            &format!("compiler-development:{mode}:{generation}"),
        ))
        .await
        .unwrap();
        let tools = crate::installed_monty_catalogue::bootstrap_compiler_tools(
            catalogue.pool.clone(),
            catalogue.kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            mode == "denied",
        )
        .unwrap();
        let root = crate::global_root_seed::retain_packaged_global_root(
            catalogue.pool.clone(),
            crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
        )
        .await
        .unwrap();
        let boot = RootBoot {
            source: root.source().to_owned(),
            checksum: root.source_checksum(),
            aliases: root.ports().iter().cloned().collect(),
            bounds: GlobalBounds {
                values: bounds,
                workers: 1,
                max_pending_calls: 8,
            },
            heap_settings: Some(HeapSettings {
                revision: 1,
                max_vm_bytes: heap,
            }),
            adapter_reserve_bytes: adapter,
            startup_timeout: Duration::from_secs(10),
            task_settings: TaskSettings {
                revision: 1,
                max_compute_time: Duration::from_secs(60),
                token_budgets_enabled: false,
            },
            max_recipe_contexts: 8,
        };
        let actor = ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: frame * 2,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: frame * 2,
        };
        let source = CompilerFlowSource {
            reference: admitted.input["user_input"].as_str().unwrap().to_owned(),
            inputs: task_inputs,
            inspected: inspected.clone(),
            tools,
            pool: catalogue.pool.clone(),
            package: package.to_owned(),
            generation: generation.clone(),
        };
        let observed = crate::bootstrap_recipe_flow::execute(
            &admitted,
            crate::bootstrap_recipe_flow::Request {
                worker: catalogue.worker.clone(),
                boot,
                process,
                actor,
                capacity: crate::global_recipe_ports::RecipeCapacity {
                    revision: 1,
                    max_recipes: 8,
                    data_limits: brassclaw_skills::value_contract::ContractLimits {
                        max_bytes: frame,
                        ..brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS
                    },
                },
                source: Arc::new(source),
            },
        )
        .await
        .unwrap();
        let success = matches!(mode, "valid" | "duplicate");
        assert_eq!(
            matches!(
                observed.receipt.outcome,
                brassclaw_monty_host::service::TaskOutcome::InternalCompleted { .. }
            ),
            success,
            "{mode}: {:?}; {}",
            observed.receipt.outcome,
            observed.report
        );
        assert_eq!(observed.report["root_completed"], json!(success));
        assert_eq!(observed.report["model_exchange_count"], json!(0));
        assert_eq!(observed.report["semantic_approval"], json!(false));
        assert!(observed.report["reply"].is_null());
        let fingerprints = observed
            .ports
            .observe_internal_execution(|execution| {
                for observation in execution.observations().values() {
                    assert!(observation.settled());
                    assert!(observation.observation_error().is_none());
                }
                if success {
                    assert!(execution.is_complete());
                    assert_eq!(execution.host_answers().len(), 5);
                    assert!(execution.transport_failure().is_none());
                } else {
                    use brassclaw_engine::executor::retained_recipe::RetainedTransportEvidence;
                    use brassclaw_monty_host::{
                        VmFailure,
                        global::Lifecycle,
                        process::{ProcessFailure, RecipeCommand, WorkerCommand},
                    };
                    assert_eq!(
                        execution.failed_step_id(),
                        Some(if mode == "missing-guide" {
                            "0:2"
                        } else {
                            "0:4"
                        })
                    );
                    let Some(RetainedTransportEvidence::Process { error, command, .. }) =
                        execution.transport_failure()
                    else {
                        panic!("expected actual contained Python failure");
                    };
                    assert!(matches!(error.kind, ProcessFailure::Vm(VmFailure::Python)));
                    let snapshot = error.snapshot.as_ref().unwrap();
                    assert_eq!(snapshot.lifecycle, Lifecycle::Ready);
                    assert_eq!(
                        snapshot.root.as_ref().unwrap().vm_id(),
                        observed.receipt.root.vm_id()
                    );
                    let command = command.as_ref().unwrap();
                    match mode {
                        "missing-guide" => {
                            assert!(execution.host_answers().is_empty());
                            assert!(matches!(
                                command.as_ref(),
                                WorkerCommand::Recipe {
                                    // A long pure stage may reach its Python rejection
                                    // after a cooperative compute-slice yield.
                                    command: RecipeCommand::Start { .. } | RecipeCommand::ResumeControl { .. },
                                    ..
                                }
                            ));
                        }
                        "denied" => {
                            assert_eq!(execution.host_answers().len(), 1);
                            assert!(matches!(
                                &execution.host_answers()[0].1,
                                PortAnswer::TerminalError { reason_kind } if reason_kind == "retained_tool_authorization"
                            ));
                            assert!(matches!(
                                command.as_ref(),
                                WorkerCommand::Recipe {
                                    command: RecipeCommand::ResumeHost { .. },
                                    ..
                                }
                            ));
                        }
                        _ => unreachable!(),
                    }
                }
                execution.observations().iter().map(|(step, observation)|
                    (step.clone(), (observation.arguments_checksum(), observation.answer_checksum())))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .await
            .unwrap();
        assert!(admitted.host.finalized_reply_ref().is_none());
        assert!(observed.exit.failure.is_none());
        assert!(observed.exit.pending_admission.is_none());
        assert!(observed.exit.queued_admissions.is_empty());
        let exit = observed.exit.transport.unwrap();
        assert!(exit.exit_status.is_some());
        assert!(
            exit.containment_error.is_none()
                && exit.reap_error.is_none()
                && exit.shutdown_failure.is_none()
        );
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
        let client = catalogue.pool.get().await.unwrap();
        let invocations = client.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&admitted.attempt.run_id.as_uuid()]).await.unwrap();
        assert_eq!(
            invocations.len(),
            if success {
                5
            } else if mode == "denied" {
                1
            } else {
                0
            }
        );
        for row in invocations {
            use brassclaw_engine::executor::retained_recipe::{
                port_answer_checksum, typed_value_checksum,
            };
            let step: String = row.get(0);
            assert!(program.bindings().contains_key(&step));
            assert_eq!(
                row.get::<_, uuid::Uuid>(1),
                program.inputs().instruction().recipe().uuid
            );
            let selected =
                crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, &step)
                    .unwrap();
            assert_eq!(row.get::<_, String>(2), selected);
            assert_eq!(
                row.get::<_, String>(3),
                hex::encode(Sha256::digest(selected.as_bytes()))
            );
            let arguments: Value = serde_json::from_str(&row.get::<_, String>(4)).unwrap();
            let answer: PortAnswer =
                serde_json::from_str(&row.get::<_, Option<String>>(5).unwrap()).unwrap();
            assert_eq!(
                fingerprints[&step],
                (
                    Some(typed_value_checksum(&arguments).unwrap()),
                    Some(port_answer_checksum(&answer).unwrap())
                )
            );
            assert_eq!(row.get::<_, String>(6), "answered");
            assert_eq!(row.get::<_, i16>(7), 1);
        }
        let row=client.query_one("SELECT generation_bytes,generation_kind FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1",&[&generation]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), package);
        assert_eq!(row.get::<_, String>(1), "compiler-package");
        let after:Value=client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb) FROM reborn_basic_prompt_store s",&[]).await.unwrap().get(0);
        assert_eq!(after, serving);
    }
}

/// Development-only selection owner. Its receipt check reads actual native
/// answers and storage; no caller-provided verdict becomes approval evidence.
struct CompilerFlowSource {
    reference: String,
    inputs: Value,
    inspected: Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>,
    tools: Arc<dyn brassclaw_engine::executor::retained_recipe::RetainedToolPort>,
    pool: Arc<brassclaw_pg::PgPool>,
    package: String,
    generation: String,
}
#[async_trait::async_trait]
impl crate::global_recipe_ports::InternalRecipeSource for CompilerFlowSource {
    fn name(&self) -> &str {
        "prefix-source-compile"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(
        &self,
    ) -> Result<
        crate::global_recipe_ports::SelectedMontyRecipe,
        brassclaw_monty_host::service::PortFailure,
    > {
        Ok(crate::global_recipe_ports::SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), brassclaw_monty_host::service::PortFailure> {
        let invalid =
            || brassclaw_monty_host::service::PortFailure::new("compiler_receipt_invalid").unwrap();
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || selected.program().inputs().instruction().recipe().uuid
                != self
                    .inspected
                    .program()
                    .inputs()
                    .instruction()
                    .recipe()
                    .uuid
            || receipt != format!("prefix-generation:base-prompt:{}", self.generation)
            || !execution.is_complete()
            || execution.host_answers().len() != 5
            || execution.transport_failure().is_some()
        {
            return Err(invalid());
        }
        let Some((_, PortAnswer::Return { value })) = execution.host_answers().last() else {
            return Err(invalid());
        };
        if value["generation"] != json!(self.generation) || value["profile"] != json!("base-prompt")
        {
            return Err(invalid());
        }
        let client = self.pool.get().await.map_err(|_| invalid())?;
        let row = client.query_opt("SELECT generation_bytes FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1", &[&self.generation]).await.map_err(|_| invalid())?.ok_or_else(invalid)?;
        if row.get::<_, String>(0) != self.package {
            return Err(invalid());
        }
        Ok(())
    }
}
