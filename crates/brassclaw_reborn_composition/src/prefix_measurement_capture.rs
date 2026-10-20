//! Resolve immutable measurement inputs from their existing owners. No design
//! acceptance, inferred runtime identity, provider dispatch or source trimming.
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{InterceptorConfigServiceError as Error, MeasurePrefixRequest};
use brassclaw_skills::value_contract::{EXECUTION_DATA_LIMITS, strict_json};
use serde_json::{Value, json};
use std::sync::Arc;

fn bad() -> Error {
    Error::InvalidRequest {
        reason: "measurement requires compatible retained source/design/model inputs and explicit probe limits".into(),
    }
}
fn read(bytes: &str) -> Result<Value, Error> {
    strict_json(bytes, EXECUTION_DATA_LIMITS).map_err(|_| bad())
}
fn file(package: &Value, name: &str) -> Result<String, Error> {
    String::from_utf8(
        STANDARD
            .decode(
                package["artifacts"][name]["data"]
                    .as_str()
                    .ok_or_else(bad)?,
            )
            .map_err(|_| bad())?,
    )
    .map_err(|_| bad())
}

/// Large contexts belong to immutable execution data, not status/metadata rows.
pub(crate) fn metadata(profile: &str, request: &MeasurePrefixRequest) -> Result<Value, Error> {
    if request.probes_json.len() > EXECUTION_DATA_LIMITS.max_bytes
        || [
            &request.generation,
            &request.proposal,
            &request.artifact_set,
        ]
        .iter()
        .any(|id| {
            id.len() != 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    {
        return Err(bad());
    }
    let capture = json!({"expected_profile_revision":request.expected_profile_revision,
        "generation":request.generation,"proposal":request.proposal,"artifact_set":request.artifact_set,
        "context_window_tokens":request.context_window_tokens,"reserved_additional_input_tokens":request.reserved_additional_input_tokens,
        "probes_sha256":brassclaw_model_encoding::checksum(request.probes_json.as_bytes())});
    Ok(json!({"profile":profile,"operation":"measure","capture":capture}))
}

/// Reconstruct only the originally captured identities, never current provider
/// selections. Compare all captured bytes before admitting a retained selection.
pub(crate) async fn verify(
    pool: Arc<PgPool>,
    definition: &crate::prefix_job_definitions::Definition,
) -> Result<(), Error> {
    let captured = read(definition.input["request_json"].as_str().ok_or_else(bad)?)?;
    let probes = captured["probes_json"].as_str().ok_or_else(bad)?;
    let mut request = definition.request["capture"].clone();
    request
        .as_object_mut()
        .ok_or_else(bad)?
        .remove("probes_sha256");
    request["probes_json"] = json!(probes);
    let request: MeasurePrefixRequest = serde_json::from_value(request).map_err(|_| bad())?;
    if request.expected_profile_revision != definition.revision
        || metadata(&definition.profile, &request)? != definition.request
        || capture(
            pool,
            definition.scope.tenant_id.as_str(),
            &definition.profile,
            &request,
        )
        .await?
            != definition.input
    {
        return Err(bad());
    }
    Ok(())
}

pub(crate) async fn capture(
    pool: Arc<PgPool>,
    namespace: &str,
    profile: &str,
    request: &MeasurePrefixRequest,
) -> Result<Value, Error> {
    if !matches!(profile, "base-prompt" | "prefix-maker")
        || request.expected_profile_revision == 0
        || !(1..=9_007_199_254_740_991).contains(&request.context_window_tokens)
        || request.reserved_additional_input_tokens > 9_007_199_254_740_991
    {
        return Err(bad());
    }
    // Text probes bypass template injection, so cannot measure this adapter.
    let probes = brassclaw_model_encoding::parse_probes(&request.probes_json).map_err(|_| bad())?;
    if probes.len() < 2
        || probes
            .iter()
            .any(|p| !matches!(p, brassclaw_model_encoding::Probe::Chat { .. }))
    {
        return Err(bad());
    }
    let store =
        crate::prefix_model_designs::PgPrefixModelDesigns::new(pool.clone(), namespace.into());
    let proposal = store.get_proposal(profile, &request.proposal).await?;
    let spec = store.get_specification(&proposal.specification).await?;
    let specification = read(&spec.specification_bytes)?;
    // A configured limit is an upper bound, not evidence that the target really
    // supports it. Unknown capacity remains an explicit diagnostic assumption.
    if let Some(configured) = specification["configured_context_window"].as_u64()
        && request.context_window_tokens > configured
    {
        return Err(bad());
    }
    let design = read(&proposal.proposal_bytes)?;
    let assets =
        crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(pool.clone(), namespace.into())
            .get(&proposal.specification, &request.artifact_set)
            .await?;
    let generation = crate::prefix_generations::PgPrefixGenerations::new(pool)
        .get(profile, &request.generation)
        .await?;
    if generation.summary.kind != "compiler-package" {
        return Err(bad());
    }
    // get() already verifies checksums, originals, maps and complete package.
    let package = read(&generation.generation_bytes)?;
    let source_design = read(&file(&package, "profile_design.json")?)?;
    if source_design["evidence_view"] != design["design"]["evidence_view"]
        || source_design["source_selection"] != design["design"]["source_selection"]
    {
        return Err(bad());
    }
    let assets = read(&assets.artifact_set_bytes)?;
    let original_template = if assets["artifacts"].get("chat_template.jinja").is_some() {
        file(&assets, "chat_template.jinja")?
    } else {
        read(&file(&assets, "tokenizer_config.json")?)?["chat_template"]
            .as_str()
            .ok_or_else(bad)?
            .to_owned()
    };
    let request_value = metadata(profile, request)?;
    let request_bytes = crate::prefix_generations::canonical(&request_value)?;
    let limits_ref = format!(
        "prefix-measurement-request:{}",
        brassclaw_model_encoding::checksum(request_bytes.as_bytes())
    );
    let captured = json!({"reference":file(&package,"context.md")?,"original_template":original_template,
        "probes_json":request.probes_json,"specification":proposal.specification,"artifact_set":request.artifact_set,
        "limits_ref":limits_ref,"context_window_tokens":request.context_window_tokens,
        "reserved_output_tokens":design["design"]["reserve_tokens"],"reserved_additional_input_tokens":request.reserved_additional_input_tokens});
    Ok(json!({"request_json":crate::prefix_generations::canonical(&captured)?}))
}

pub(crate) async fn select(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    host: Arc<brassclaw_reborn::monty_task_host::MontyTaskHost>,
    admission: Arc<crate::pg_monty_admission::PgMontyAdmission>,
    definition: crate::prefix_job_definitions::Definition,
    reference: String,
) -> Result<
    Arc<dyn crate::global_recipe_ports::InternalRecipeSource>,
    brassclaw_monty_host::service::PortFailure,
> {
    use brassclaw_engine::executor::retained_recipe::RetainedProgram;
    let failed = || {
        brassclaw_monty_host::service::PortFailure::new("prefix_measurement_selection_invalid")
            .expect("constant classification")
    };
    verify(catalogue.pool.clone(), &definition)
        .await
        .map_err(|_| failed())?;
    let qualification = catalogue
        .measurement_qualification
        .as_ref()
        .ok_or_else(failed)?;
    let inspected = qualification.program();
    if inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(|_| failed())?
        .exact_bytes()
        != definition.program_bytes
    {
        return Err(failed());
    }
    let mut client = catalogue.pool.get().await.map_err(|_| failed())?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|_| failed())?;
    qualification
        .verify_in_transaction(&tx)
        .await
        .map_err(|_| failed())?;
    tx.commit().await.map_err(|_| failed())?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(failed());
    };
    let tools = crate::installed_monty_catalogue::bootstrap_measurement_tools(
        catalogue.pool.clone(),
        catalogue.kernel.clone(),
        host,
        admission,
        program.clone(),
        false,
    )
    .map_err(|_| failed())?;
    Ok(Arc::new(crate::prefix_job_execution::Invocation {
        definition,
        reference,
        inspected,
        tools,
        pool: catalogue.pool.clone(),
    }))
}

/// Validate the actual completed workflow's subject and immutable native journal.
/// Capacity decisions are made by the selected PythonCode, never by this adapter.
pub(crate) async fn verify_receipt(
    definition: &crate::prefix_job_definitions::Definition,
    receipt: &str,
    selected: &brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
    execution: &brassclaw_engine::executor::retained_recipe::RetainedRecipeExecution,
    pool: &Arc<PgPool>,
) -> Result<(), brassclaw_monty_host::service::PortFailure> {
    use brassclaw_engine::executor::retained_recipe::{RetainedProgram, port_answer_checksum};
    use brassclaw_monty_host::process::PortAnswer;
    let failed = || {
        brassclaw_monty_host::service::PortFailure::new("prefix_measurement_receipt_invalid")
            .expect("constant classification")
    };
    let RetainedProgram::Tools(program) = selected.program() else {
        return Err(failed());
    };
    if !execution.is_complete()
        || execution.transport_failure().is_some()
        || execution.host_answers().len() != 2
    {
        return Err(failed());
    }
    let value = read(receipt).map_err(|_| failed())?;
    program
        .inputs()
        .validate_step_result("0:9", &value)
        .map_err(|_| failed())?;
    let input = read(
        definition.input["request_json"]
            .as_str()
            .ok_or_else(failed)?,
    )
    .map_err(|_| failed())?;
    if crate::prefix_generations::canonical(&value).map_err(|_| failed())? != receipt
        || value["format"] != "brassclaw-prefix-capacity-comparison/1"
        || value["qualification"] != "calculation-only; not approval or target verification"
        || value["cache_qualification"] != "unverified"
        || value["model_quality"] != "unmeasured"
    {
        return Err(failed());
    }
    for field in [
        "specification",
        "artifact_set",
        "limits_ref",
        "context_window_tokens",
        "reserved_output_tokens",
        "reserved_additional_input_tokens",
    ] {
        if value[field] != input[field] {
            return Err(failed());
        }
    }
    let client = pool.get().await.map_err(|_| failed())?;
    let rows = client.query("SELECT step_id,selection_bytes,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id", &[&definition.id]).await.map_err(|_| failed())?;
    if rows.len() != 2 {
        return Err(failed());
    }
    for (row, (step, actual)) in rows.into_iter().zip(execution.host_answers()) {
        if row.get::<_, String>(0) != *step
            || row.get::<_, String>(4) != "answered"
            || row.get::<_, i16>(5) != 1
            || row.get::<_, String>(1)
                != crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, step)
                    .map_err(|_| failed())?
        {
            return Err(failed());
        }
        let recorded: PortAnswer = serde_json::from_str(
            row.get::<_, Option<String>>(3)
                .as_deref()
                .ok_or_else(failed)?,
        )
        .map_err(|_| failed())?;
        if port_answer_checksum(&recorded).map_err(|_| failed())?
            != port_answer_checksum(actual).map_err(|_| failed())?
        {
            return Err(failed());
        }
        let PortAnswer::Return { value: output } = actual else {
            return Err(failed());
        };
        let arguments = read(&row.get::<_, String>(2)).map_err(|_| failed())?;
        match step.as_str() {
            "0:5" => {
                let texts =
                    read(arguments["data"].as_str().ok_or_else(failed)?).map_err(|_| failed())?;
                if arguments["operation"] != "sha256_utf8"
                    || texts.as_array().is_none_or(|texts| texts.len() != 2)
                    || texts[1] != input["original_template"]
                    || brassclaw_model_encoding::checksum(
                        texts[0].as_str().ok_or_else(failed)?.as_bytes(),
                    ) != value["template_sha256"]
                    || output["digests"][0] != value["template_sha256"]
                    || output["digests"][1]
                        != brassclaw_model_encoding::checksum(
                            input["original_template"]
                                .as_str()
                                .ok_or_else(failed)?
                                .as_bytes(),
                        )
                {
                    return Err(failed());
                }
            }
            "0:7" => {
                let observed = read(output.as_str().ok_or_else(failed)?).map_err(|_| failed())?;
                if arguments["specification"] != input["specification"]
                    || arguments["artifact_set"] != input["artifact_set"]
                    || arguments["probes_json"] != input["probes_json"]
                    || brassclaw_model_encoding::checksum(
                        arguments["template_utf8"]
                            .as_str()
                            .ok_or_else(failed)?
                            .as_bytes(),
                    ) != value["template_sha256"]
                    || observed["report"]["chat_template_sha256"] != value["template_sha256"]
                    || observed["report"]["common_prefix_tokens"] != value["common_prefix_tokens"]
                    || observed["worker_checksum"] != value["worker_checksum"]
                    || observed["preparation_revision"] != value["preparation_revision"]
                {
                    return Err(failed());
                }
            }
            _ => return Err(failed()),
        }
    }
    Ok(())
}
