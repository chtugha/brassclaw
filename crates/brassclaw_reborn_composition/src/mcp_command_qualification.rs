//! Catalogue-owned qualification. It observes existing durable normal-chat
//! executions and probes the real matcher; it never runs a command or grants
//! activation/Tool authority. No public constructor accepts a success report.
use std::collections::{BTreeMap, BTreeSet};

use brassclaw_engine::{
    executor::{
        retained_preload::PreloadDeclaration, retained_recipe::RetainedProgram,
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        intent_system::{
            IntentResolution, IntentScope, RetainedIntentEligibility,
            resolve_catalogue_intent_in_transaction,
        },
        retained_instruction::{RetainedRecipeInstruction, compile_matched_retained_recipe},
        template_extractor::parse_template,
    },
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS, revision_store::PgComponentRevisionStore,
    value_contract::validate_data_bounds,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

use crate::mcp_recipe_catalogue::{CommandContract, McpDiscoveryError, McpRecipeDiscovery};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandCases {
    format: String,
    success_examples: Vec<SuccessCase>,
    failure_examples: Vec<FailureCase>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SuccessCase {
    command: String,
    reply: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureCase {
    command: String,
    reason_kind: String,
    tool_failure_kind: String,
}

/// Created only from checked database observations after durable commit.
/// It is not an approval receipt and cannot be deserialized from client JSON.
pub(crate) struct QualifiedCommands {
    generation: Uuid,
    contracts: BTreeMap<String, [u8; 32]>,
    checksum: [u8; 32],
}
impl QualifiedCommands {
    pub(crate) fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub(crate) fn verify(
        &self,
        generation: Uuid,
        contracts: &BTreeMap<String, CommandContract>,
    ) -> Result<(), McpDiscoveryError> {
        if self.generation != generation
            || self.contracts
                != contracts
                    .iter()
                    .map(|(name, contract)| (name.clone(), contract.checksum))
                    .collect()
        {
            return Err(McpDiscoveryError::Unqualified);
        }
        Ok(())
    }
}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(bytes.as_ref()))
}
fn unavailable<T>(_: T) -> McpDiscoveryError {
    McpDiscoveryError::Unavailable
}

/// A conservative language intersection check for the matcher's SQL LIKE
/// grammar. Compatible literal prefixes AND suffixes may intersect; decline
/// qualification rather than rely on scores/row order or a finite sample.
fn may_overlap(left: &str, right: &str) -> bool {
    let (lp, ls) = (
        left.split('%').next().unwrap(),
        left.rsplit('%').next().unwrap(),
    );
    let (rp, rs) = (
        right.split('%').next().unwrap(),
        right.rsplit('%').next().unwrap(),
    );
    (lp.starts_with(rp) || rp.starts_with(lp)) && (ls.ends_with(rs) || rs.ends_with(ls))
}

/// Used by the normal installation owner after package/integrity qualification.
/// `routing` is its entire incoming eligible catalogue, excluding named-only
/// helpers. No drafts, legacy-row inference or per-call execution port is added.
pub(crate) async fn qualify_installed_commands(
    pool: &PgPool,
    scope: &IntentScope,
    generation: Uuid,
    programs: &[&InspectedRetainedProgram],
    routing: &[&InspectedRetainedProgram],
    approval: Option<&crate::bootstrap_reply_approval::BootstrapReplyApproval>,
) -> Result<QualifiedCommands, McpDiscoveryError> {
    let contracts = McpRecipeDiscovery::contracts(generation, programs)?;
    let mut client = pool.get().await.map_err(unavailable)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(unavailable)?;
    let catalogue = tx.query_one("SELECT catalogue_bytes,checksum FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1", &[&generation]).await.map_err(unavailable)?;
    let catalogue_bytes: String = catalogue.get(0);
    if digest(&catalogue_bytes) != catalogue.get::<_, String>(1) {
        return Err(McpDiscoveryError::Unqualified);
    }
    let instructions: Vec<_> = routing
        .iter()
        .map(|program| program.program().inputs().instruction())
        .collect();
    let eligible =
        RetainedIntentEligibility::from_instructions(&instructions).map_err(unavailable)?;
    let routing_metadata_checksum = verify_routing_metadata(&tx, scope, &instructions).await?;
    let mut routing_selections = BTreeSet::new();
    for selected in programs.iter().chain(routing.iter()) {
        let instruction = selected.program().inputs().instruction();
        let references: Vec<_> = instruction
            .snapshot()
            .revisions()
            .values()
            .map(|revision| revision.reference())
            .collect();
        PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[instruction.recipe().uuid],
            &references,
        )
        .await
        .map_err(unavailable)?;
    }
    for instruction in &instructions {
        routing_selections.insert(hex::encode(
            instruction
                .retained_selection()
                .map_err(unavailable)?
                .checksum(),
        ));
    }
    if !contracts.is_empty() {
        approval
            .ok_or(McpDiscoveryError::Unqualified)?
            .verify_in_transaction(&tx)
            .await
            .map_err(unavailable)?;
    }
    let mut proofs = BTreeMap::new();
    for (name, contract) in &contracts {
        let selected = programs
            .iter()
            .find(|program| {
                program
                    .program()
                    .inputs()
                    .instruction()
                    .snapshot()
                    .revisions()[&program.program().inputs().instruction().recipe().uuid]
                    .draft()
                    .document()
                    .get("mcp_call")
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str)
                    == Some(name)
            })
            .ok_or(McpDiscoveryError::Contract)?;
        let instruction = selected.program().inputs().instruction();
        let selection = instruction.retained_selection().map_err(unavailable)?;
        if !routing_selections.contains(&hex::encode(selection.checksum())) {
            return Err(McpDiscoveryError::Unqualified);
        }
        approval
            .ok_or(McpDiscoveryError::Unqualified)?
            .require_selected_program(selected)
            .map_err(unavailable)?;
        let catalogue_document: Value =
            serde_json::from_str(&catalogue_bytes).map_err(|_| McpDiscoveryError::Unqualified)?;
        let retained_reply_bytes = serde_json::to_string(&catalogue_document["reply_selection"])
            .map_err(|_| McpDiscoveryError::Unqualified)?;
        if retained_reply_bytes != selection.exact_bytes() {
            return Err(McpDiscoveryError::Unqualified);
        }
        check_export(selected, contract)?;
        for candidate in &instructions {
            for template in &candidate.variant().intent_examples {
                if template == &contract.declaration.sentence
                    && candidate.recipe() == instruction.recipe()
                    && candidate.variant().variant_key == instruction.variant().variant_key
                {
                    continue;
                }
                if may_overlap(&contract.declaration.sentence, template) {
                    return Err(McpDiscoveryError::Conflict);
                }
            }
        }
        let document = instruction.snapshot().revisions()[&instruction.recipe().uuid]
            .draft()
            .document();
        let cases: CommandCases = serde_json::from_value(
            document
                .get("mcp_qualification")
                .cloned()
                .ok_or(McpDiscoveryError::Contract)?,
        )
        .map_err(|_| McpDiscoveryError::Contract)?;
        let subject = CommandSubject {
            scope,
            generation,
            selected,
            contract,
            eligible: &eligible,
        };
        proofs.insert(name.clone(), check_cases(&tx, &subject, &cases).await?);
    }
    let evidence = json!({"format":"mcp-command-qualification/1","catalogue_generation":generation,
        "scope":{"tenant_id":scope.tenant_id,"user_id":scope.user_id,"agent_id":scope.agent_id,"project_id":scope.project_id},
        "routing_selections":routing_selections,"routing_metadata_checksum":routing_metadata_checksum,"commands":proofs,"semantic_approval":false,"catalogue_activation":false});
    validate_data_bounds(&evidence, REVISION_LIMITS).map_err(|_| McpDiscoveryError::Contract)?;
    let bytes = evidence.to_string();
    if bytes.len() > REVISION_LIMITS.max_bytes {
        return Err(McpDiscoveryError::Contract);
    }
    let checksum: [u8; 32] = Sha256::digest(bytes.as_bytes()).into();
    let encoded = hex::encode(checksum);
    tx.execute("INSERT INTO brassclaw_mcp_command_qualifications(checksum,catalogue_id,evidence_bytes) VALUES($1,$2,$3) ON CONFLICT(checksum) DO NOTHING", &[&encoded,&generation,&bytes]).await.map_err(unavailable)?;
    let actual = tx.query_one("SELECT catalogue_id,evidence_bytes FROM brassclaw_mcp_command_qualifications WHERE checksum=$1", &[&encoded]).await.map_err(unavailable)?;
    if actual.get::<_, Uuid>(0) != generation || actual.get::<_, String>(1) != bytes {
        return Err(McpDiscoveryError::Unqualified);
    }
    tx.commit().await.map_err(unavailable)?;
    Ok(QualifiedCommands {
        generation,
        contracts: contracts
            .into_iter()
            .map(|(name, contract)| (name, contract.checksum))
            .collect(),
        checksum,
    })
}

/// Check the physical eligible rows, not just component prose or a few samples.
/// A missing/corrupt canonical route is an error, never manufactured No-Match.
pub(crate) async fn verify_routing_metadata(
    tx: &Transaction<'_>,
    scope: &IntentScope,
    instructions: &[&RetainedRecipeInstruction],
) -> Result<String, McpDiscoveryError> {
    let mut observed = BTreeMap::new();
    for instruction in instructions {
        let id = instruction.recipe().uuid;
        for template in &instruction.variant().intent_examples {
            let (is_template, prefix, suffix) = match parse_template(template) {
                Some((prefix, suffix)) => (true, Some(prefix), Some(suffix)),
                None => (false, None, None),
            };
            let rows = tx.query("SELECT input_class,is_template,template_prefix,template_suffix FROM reborn_intent_inputs WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 AND component_class_code=21 AND step_link=$6 AND input_text=$7 AND needs_review=false ORDER BY input_class", &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&id,&instruction.variant().step_link,&template]).await.map_err(unavailable)?;
            if rows.is_empty()
                || rows.iter().any(|row| {
                    row.get::<_, bool>(1) != is_template
                        || row.get::<_, Option<String>>(2) != prefix
                        || row.get::<_, Option<String>>(3) != suffix
                })
            {
                return Err(McpDiscoveryError::Unqualified);
            }
            observed.insert(format!("{id}:{}:{template}", instruction.variant().variant_key),
                json!({"step_link":instruction.variant().step_link,"is_template":is_template,"prefix":prefix,"suffix":suffix,
                    "classes":rows.iter().map(|row| row.get::<_,i16>(0)).collect::<Vec<_>>()}));
        }
    }
    Ok(digest(
        serde_json::to_vec(&observed).map_err(|_| McpDiscoveryError::Contract)?,
    ))
}

async fn check_cases(
    tx: &Transaction<'_>,
    subject: &CommandSubject<'_>,
    cases: &CommandCases,
) -> Result<Value, McpDiscoveryError> {
    let CommandSubject {
        scope,
        selected,
        contract,
        eligible,
        ..
    } = subject;
    let selection = selected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(unavailable)?;
    if cases.format != "mcp-command-cases/1"
        || cases.success_examples.len() != contract.declaration.examples.len()
        || cases.failure_examples.is_empty()
        || cases.failure_examples.len() > 64
    {
        return Err(McpDiscoveryError::Contract);
    }
    let mut commands: BTreeSet<_> = contract
        .declaration
        .examples
        .iter()
        .chain(&contract.declaration.negative_examples)
        .map(String::as_str)
        .collect();
    let mut successes = Vec::new();
    let mut observed_successes = BTreeSet::new();
    for case in &cases.success_examples {
        if !contract.declaration.examples.contains(&case.command)
            || !observed_successes.insert(&case.command)
        {
            return Err(McpDiscoveryError::Contract);
        }
        let observed = check_execution(tx, subject, &case.command, None).await?;
        if observed["reply_content_checksum"] != digest(&case.reply) {
            return Err(McpDiscoveryError::Unqualified);
        }
        successes.push(observed);
    }
    let mut failures = Vec::new();
    for case in &cases.failure_examples {
        if !commands.insert(&case.command)
            || case.tool_failure_kind != "retained_tool_authorization"
            || case.reason_kind.is_empty()
            || case.reason_kind.len() > 128
            || !case
                .reason_kind
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(McpDiscoveryError::Contract);
        }
        failures.push(check_execution(tx, subject, &case.command, Some(case)).await?);
    }
    let mut negatives = Vec::new();
    for command in &contract.declaration.negative_examples {
        if contract.bind(command).is_ok()
            || !matches!(
                resolve_catalogue_intent_in_transaction(tx, scope, command, eligible)
                    .await
                    .map_err(unavailable)?,
                IntentResolution::NoMatch
            )
        {
            return Err(McpDiscoveryError::Unqualified);
        }
        negatives.push(digest(command));
    }
    Ok(
        json!({"contract_checksum":hex::encode(contract.checksum), "selection_checksum":hex::encode(selection.checksum()),
        "preload_order":selected.preload_order(), "successes":successes,"failures":failures,"negative_commands":negatives}),
    )
}

fn check_export(
    selected: &InspectedRetainedProgram,
    contract: &CommandContract,
) -> Result<(), McpDiscoveryError> {
    let RetainedProgram::Tools(program) = selected.program() else {
        return Err(McpDiscoveryError::UnsupportedRunner);
    };
    let (step, binding) = program
        .bindings()
        .iter()
        .find(|(_, binding)| binding.skill().uuid == contract.declaration.skill_uuid)
        .ok_or(McpDiscoveryError::Contract)?;
    let graph = program.inputs().instruction().snapshot().revisions();
    let revision = &graph[&binding.python().uuid];
    let declaration =
        PreloadDeclaration::parse(revision.draft().document(), revision.draft().dependencies())
            .map_err(|_| McpDiscoveryError::Contract)?
            .ok_or(McpDiscoveryError::UnsupportedRunner)?;
    let invocation = declaration
        .invocation(
            &contract.declaration.export_name,
            &revision.draft().document()["input_contract"],
        )
        .map_err(|_| McpDiscoveryError::Contract)?;
    if selected.invocation(step) != Some(invocation.as_str())
        || selected.preload_order().is_empty()
        || program
            .program()
            .steplist
            .iter()
            .any(|step| selected.invocation(&step.step_id).is_none())
    {
        return Err(McpDiscoveryError::UnsupportedRunner);
    }
    Ok(())
}

struct CommandSubject<'a> {
    scope: &'a IntentScope,
    generation: Uuid,
    selected: &'a InspectedRetainedProgram,
    contract: &'a CommandContract,
    eligible: &'a RetainedIntentEligibility,
}

async fn check_execution(
    tx: &Transaction<'_>,
    subject: &CommandSubject<'_>,
    command: &str,
    failure: Option<&FailureCase>,
) -> Result<Value, McpDiscoveryError> {
    let CommandSubject {
        scope,
        generation,
        selected,
        contract,
        eligible,
    } = subject;
    let inputs = contract.bind(command)?;
    let instruction = selected.program().inputs().instruction();
    let matched = resolve_catalogue_intent_in_transaction(tx, scope, command, eligible)
        .await
        .map_err(unavailable)?;
    let refs: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|revision| revision.reference())
        .collect();
    let snapshot = PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[instruction.recipe().uuid],
        &refs,
    )
    .await
    .map_err(unavailable)?;
    let actual = compile_matched_retained_recipe(
        std::sync::Arc::new(snapshot),
        &matched,
        instruction.class(),
    )
    .map_err(|_| McpDiscoveryError::Unqualified)?;
    let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(original) =
        selected.program()
    else {
        return Err(McpDiscoveryError::Unqualified);
    };
    let actual = brassclaw_engine::memory::retained_tools::prepare_retained_tool_program(actual)
        .and_then(|actual| actual.retain_approvals_from(original))
        .map_err(unavailable)?;
    let selection = instruction.retained_selection().map_err(unavailable)?;
    let IntentResolution::Match { input_text, .. } = &matched else {
        return Err(McpDiscoveryError::Unqualified);
    };
    if input_text != &contract.declaration.sentence
        || actual
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(unavailable)?
            .checksum()
            != selection.checksum()
        || selected
            .program()
            .inputs()
            .bind_variant_example(input_text, command, &json!({}))
            .map_err(|_| McpDiscoveryError::Contract)?
            != inputs
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    let reference = instruction.recipe();
    let matching = json!({"format":"monty-normal-match/1","catalogue_generation":generation,"command_checksum":digest(command),
        "matched_template_checksum":digest(input_text),"task_inputs_checksum":digest(inputs.to_string()),
        "recipe_uuid":reference.uuid,"recipe_version":reference.version,"recipe_checksum":hex::encode(reference.checksum),
        "variant_key":instruction.variant().variant_key,"step_link":instruction.variant().step_link,"selection_checksum":hex::encode(selection.checksum())});
    let filter = json!([{"normal_match":{"matching":matching}}]);
    let status = if failure.is_some() {
        "failed"
    } else {
        "completed"
    };
    let failure_kind = failure.map(|case| case.reason_kind.as_str());
    let host_failure = failure.map(|case| {
        json!({"kind":"terminal_error", "reason_kind":case.tool_failure_kind}).to_string()
    });
    let row = tx.query_opt("SELECT a.run_id,a.outcome,s.selection_bytes FROM brassclaw_monty_task_admissions a JOIN brassclaw_monty_recipe_selections s ON s.run_id=a.run_id AND s.recipe_id=$1 WHERE a.phase='settled' AND a.scope->>'tenant_id'=$2 AND a.scope->>'agent_id'=$3 AND COALESCE(a.scope->>'project_id','default')=$4 AND a.outcome->>'status'=$5 AND a.outcome->'execution'->'recipes' @> $6::jsonb AND ($7::text IS NULL OR a.outcome->>'reason_kind'=$7) AND s.selection_checksum=$8 AND ($9::text IS NULL OR EXISTS(SELECT 1 FROM brassclaw_monty_tool_invocations i WHERE i.run_id=a.run_id AND i.recipe_id=$1 AND i.answer_bytes::jsonb=$9::jsonb)) ORDER BY a.run_id LIMIT 1", &[&reference.uuid,&scope.tenant_id,&scope.agent_id,&scope.project_id,&status,&filter,&failure_kind,&hex::encode(selection.checksum()),&host_failure]).await.map_err(unavailable)?.ok_or(McpDiscoveryError::Unqualified)?;
    let outcome: Value = row.get(1);
    if row.get::<_, String>(2) != selection.exact_bytes() {
        return Err(McpDiscoveryError::Unqualified);
    }
    check_settlement(selected, &matching, &outcome, failure_kind)?;
    let invocation = check_invocation(tx, selected, row.get(0), &inputs, &outcome, failure).await?;
    Ok(
        json!({"command_checksum":digest(command),"run_id":row.get::<_,Uuid>(0),"outcome_checksum":digest(outcome.to_string()),"reply_ref":outcome.get("reply_ref"),"reply_content_checksum":outcome["execution"]["reply"]["content_checksum"],"reason_kind":failure_kind,"invocation":invocation,"root":outcome["execution"]["root"]}),
    )
}

/// Reconcile actual dispatch intent, pinned selection, typed arguments and host
/// answer. A generic Recipe failure cannot stand in for a kernel policy denial.
async fn check_invocation(
    tx: &Transaction<'_>,
    selected: &InspectedRetainedProgram,
    run: Uuid,
    inputs: &Value,
    outcome: &Value,
    failure: Option<&FailureCase>,
) -> Result<Value, McpDiscoveryError> {
    use brassclaw_engine::memory::typed_bindings::PreparedReference;
    use brassclaw_monty_host::process::PortAnswer;
    let RetainedProgram::Tools(program) = selected.program() else {
        return Err(McpDiscoveryError::UnsupportedRunner);
    };
    // The currently qualified public profile is one usage, with task/constant
    // inputs. Result-dependent/multi-usage commands need their own acceptance.
    if program.bindings().len() != 1 {
        return Err(McpDiscoveryError::UnsupportedRunner);
    }
    let (step, binding) = program
        .bindings()
        .first_key_value()
        .ok_or(McpDiscoveryError::Unqualified)?;
    let mut locals = serde_json::Map::new();
    for (name, reference) in program
        .inputs()
        .layout()
        .steps()
        .get(step)
        .ok_or(McpDiscoveryError::Unqualified)?
    {
        let value = match reference {
            PreparedReference::Input { name } => inputs
                .get(name)
                .ok_or(McpDiscoveryError::Unqualified)?
                .clone(),
            PreparedReference::Constant { value } => value.clone(),
            PreparedReference::Result { .. } => return Err(McpDiscoveryError::UnsupportedRunner),
        };
        locals.insert(name.clone(), value);
    }
    let locals = program
        .inputs()
        .bind_step_inputs(step, &Value::Object(locals))
        .map_err(|_| McpDiscoveryError::Unqualified)?;
    let rows = tx.query("SELECT i.invocation_id,i.step_id,i.selection_bytes,i.selection_checksum,i.arguments_bytes,i.arguments_checksum,i.answer_bytes,i.answer_checksum,i.phase,i.attempt_count,a.scope FROM brassclaw_monty_tool_invocations i JOIN brassclaw_monty_task_admissions a USING(run_id) WHERE i.run_id=$1 AND i.recipe_id=$2", &[&run, &program.inputs().instruction().recipe().uuid]).await.map_err(unavailable)?;
    if rows.len() != 1 {
        return Err(McpDiscoveryError::Unqualified);
    }
    let row = &rows[0];
    let selection =
        crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, step)
            .map_err(unavailable)?;
    let arguments: String = row.get(4);
    let answer: String = row
        .get::<_, Option<String>>(6)
        .ok_or(McpDiscoveryError::Unqualified)?;
    if row.get::<_, String>(1) != *step
        || row.get::<_, String>(2) != selection
        || row.get::<_, String>(3) != digest(&selection)
        || row.get::<_, String>(5) != digest(&arguments)
        || row.get::<_, Option<String>>(7).as_deref() != Some(digest(&answer).as_str())
        || row.get::<_, String>(8) != "answered"
        || row.get::<_, i16>(9) != 1
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    let arguments = brassclaw_skills::value_contract::strict_json(&arguments, REVISION_LIMITS)
        .map_err(|_| McpDiscoveryError::Unqualified)?;
    binding
        .bind_tool_arguments(&locals, &arguments)
        .map_err(|_| McpDiscoveryError::Unqualified)?;
    let actual: PortAnswer =
        serde_json::from_str(&answer).map_err(|_| McpDiscoveryError::Unqualified)?;
    match (actual, failure) {
        (PortAnswer::Return { value }, None) if value == outcome["reply_ref"] => {
            program
                .inputs()
                .validate_step_result(step, &value)
                .map_err(|_| McpDiscoveryError::Unqualified)?;
        }
        (PortAnswer::TerminalError { reason_kind }, Some(case))
            if reason_kind == case.tool_failure_kind && outcome["reply_ref"].is_null() => {}
        _ => return Err(McpDiscoveryError::Unqualified),
    }
    let scope: Value = row.get(10);
    let tenant = scope["tenant_id"]
        .as_str()
        .ok_or(McpDiscoveryError::Unqualified)?;
    let thread = scope["thread_id"]
        .as_str()
        .ok_or(McpDiscoveryError::Unqualified)?;
    let transcript: Value = tx
        .query_one(
            "SELECT metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&tenant, &thread],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    let run = run.to_string();
    let replies: Vec<_> = transcript["messages"]
        .as_array()
        .ok_or(McpDiscoveryError::Unqualified)?
        .iter()
        .filter(|message| {
            message["turn_run_id"] == run
                && message["kind"] == "assistant"
                && message["status"] == "finalized"
        })
        .collect();
    if failure.is_some() {
        if !replies.is_empty() {
            return Err(McpDiscoveryError::Unqualified);
        }
    } else if replies.len() != 1
        || outcome["reply_ref"]
            != format!(
                "msg:{}",
                replies[0]["message_id"]
                    .as_str()
                    .ok_or(McpDiscoveryError::Unqualified)?
            )
        || outcome["execution"]["reply"]["content_checksum"]
            != digest(
                replies[0]["content"]
                    .as_str()
                    .ok_or(McpDiscoveryError::Unqualified)?,
            )
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    Ok(
        json!({"invocation_id":row.get::<_,Uuid>(0),"selection_checksum":digest(&selection),
        "arguments_checksum":row.get::<_,String>(5),"answer_checksum":digest(&answer),"attempt_count":1}),
    )
}

fn check_settlement(
    selected: &InspectedRetainedProgram,
    matching: &Value,
    outcome: &Value,
    failure: Option<&str>,
) -> Result<(), McpDiscoveryError> {
    let execution = &outcome["execution"];
    let success = failure.is_none();
    let recipes = execution["recipes"]
        .as_array()
        .ok_or(McpDiscoveryError::Unqualified)?;
    let matched: Vec<_> = recipes
        .iter()
        .filter(|recipe| !recipe["normal_match"].is_null())
        .collect();
    let steps: Vec<_> = selected
        .program()
        .program()
        .steplist
        .iter()
        .map(|step| step.step_id.as_str())
        .collect();
    if execution["format"] != "monty-task-execution/1"
        || execution["intent_outcome"] != "match"
        || execution["root_completed"] != success
        || execution["semantic_approval"] != false
        || execution["catalogue_activation"] != false
        || execution["withheld_root_answers"] != 0
        || execution["withheld_host_answers"] != false
        || execution["accounting"]["compute_time"].is_null()
        || matched.len() != 1
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    let recipe = matched[0];
    if recipe["normal_match"]["matching"] != *matching
        || recipe["normal_match"]["task_completed"] != success
        || recipe["composed"] != true
        || recipe["pending_step"] != Value::Null
        || recipe["recipe_id"] != matching["recipe_uuid"]
        || recipe["selection_checksum"] != matching["selection_checksum"]
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    let completed: Vec<_> = recipe["completed_steps"]
        .as_array()
        .ok_or(McpDiscoveryError::Unqualified)?
        .iter()
        .map(|step| step.as_str().ok_or(McpDiscoveryError::Unqualified))
        .collect::<Result<_, _>>()?;
    if success {
        if recipe["complete"] != true
            || recipe["normal_match"]["recipe_completed"] != true
            || recipe["failed_step"] != Value::Null
            || completed != steps
            || execution["all_selected_recipes_complete"] != true
            || execution["accounting"]["failure"] != Value::Null
            || outcome["reply_ref"]
                .as_str()
                .is_none_or(|reference| brassclaw_turns::LoopMessageRef::new(reference).is_err())
            || execution["reply"]["reference"] != outcome["reply_ref"]
            || execution["reply"]["content_checksum"]
                .as_str()
                .is_none_or(|value| value.len() != 64 || hex::decode(value).is_err())
        {
            return Err(McpDiscoveryError::Unqualified);
        }
    } else if recipe["complete"] != false
        || recipe["normal_match"]["recipe_completed"] != false
        || completed.len() >= steps.len()
        || !steps.starts_with(&completed)
        || recipe["failed_step"].as_str() != Some(steps[completed.len()])
        || outcome["reason_kind"].as_str() != failure
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    let root = &execution["root"];
    if root["format"] != "monty-root-execution/1"
        || root["vm_id"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
            .is_none_or(|id| id.is_nil())
        || root["workers"].as_u64().is_none_or(|count| count == 0)
        || ["source_checksum", "aliases_checksum"].iter().any(|name| {
            root[*name]
                .as_str()
                .is_none_or(|value| value.len() != 64 || hex::decode(value).is_err())
        })
    {
        return Err(McpDiscoveryError::Unqualified);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn assert_recorded_command_cases(
    pool: &PgPool,
    scope: &IntentScope,
    generation: Uuid,
    selected: &InspectedRetainedProgram,
    positive: &str,
) {
    // Uses the actual retained normal-chat selection, worker inspection and
    // durable executions from the runtime regression. This does not opt an
    // internal Recipe into MCP or forge a public-candidate approval.
    let instruction = selected.program().inputs().instruction();
    let RetainedProgram::Tools(program) = selected.program() else {
        panic!("real Tool usage required")
    };
    let binding = program.bindings().values().next().unwrap();
    let contract = CommandContract::parse(&json!({"format":"mcp-call-skill-recipe/1","name":"reply_case",
        "purpose":"Publish this task's literal reply.","skill_uuid":binding.skill().uuid,
        "variant_key":instruction.variant().variant_key,"export_name":"publish_turn_reply",
        "sentence":"reply %","variables":[{"name":"answer","position":0,"encoding":"verbatim"}],
        "formatting_rules":"Use the exact literal prefix and a nonempty value.","examples":[positive],
        "negative_examples":["outside reply command"],"result_description":"The published reply.","error_description":"Stop on policy denial."}),
        &instruction.snapshot().revisions()[&instruction.recipe().uuid].draft().document()["input_layouts"][&instruction.variant().variant_key]["task_inputs"]).unwrap();
    check_export(selected, &contract).unwrap();
    let eligible = RetainedIntentEligibility::from_instructions(&[instruction]).unwrap();
    let subject = CommandSubject {
        scope,
        generation,
        selected,
        contract: &contract,
        eligible: &eligible,
    };
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let success = check_execution(&tx, &subject, positive, None)
        .await
        .unwrap();
    assert!(success["reply_ref"].is_string());
    let mut cases = CommandCases {
        format: "mcp-command-cases/1".into(),
        success_examples: vec![SuccessCase {
            command: positive.into(),
            reply: positive.strip_prefix("reply ").unwrap().into(),
        }],
        failure_examples: vec![FailureCase {
            command: "reply must not publish".into(),
            reason_kind: "recipe_execution_failed".into(),
            tool_failure_kind: "retained_tool_authorization".into(),
        }],
    };
    let suite = check_cases(&tx, &subject, &cases).await.unwrap();
    assert_eq!(suite["successes"].as_array().unwrap().len(), 1);
    assert_eq!(suite["failures"].as_array().unwrap().len(), 1);
    assert_eq!(suite["negative_commands"].as_array().unwrap().len(), 1);
    cases.success_examples[0].reply = "a different formatted reply".into();
    assert!(
        matches!(
            check_cases(&tx, &subject, &cases).await,
            Err(McpDiscoveryError::Unqualified)
        ),
        "a completed run with the wrong formatted reply cannot qualify"
    );
    check_execution(
        &tx,
        &subject,
        "reply must not publish",
        Some(&cases.failure_examples[0]),
    )
    .await
    .unwrap();
    assert!(
        matches!(
            check_execution(&tx, &subject, "reply must not publish", None).await,
            Err(McpDiscoveryError::Unqualified)
        ),
        "failed command cannot qualify as a successful example"
    );
    assert!(
        matches!(
            check_execution(
                &tx,
                &subject,
                "reply never executed qualification example",
                None
            )
            .await,
            Err(McpDiscoveryError::Unqualified)
        ),
        "unobserved commands cannot borrow another example's receipt"
    );
    let stale = CommandSubject {
        generation: Uuid::new_v4(),
        ..subject
    };
    assert!(
        matches!(
            check_execution(&tx, &stale, positive, None).await,
            Err(McpDiscoveryError::Unqualified)
        ),
        "old execution must not qualify a new generation"
    );
    assert!(matches!(
        resolve_catalogue_intent_in_transaction(&tx, scope, "outside reply command", &eligible)
            .await
            .unwrap(),
        IntentResolution::NoMatch
    ));
    tx.commit().await.unwrap();
    let routing_tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    verify_routing_metadata(&routing_tx, scope, &[instruction])
        .await
        .unwrap();
    let changed = routing_tx.execute(
        "UPDATE reborn_intent_inputs SET template_prefix='corrupt route ' WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 AND step_link=$6 AND input_text='reply %'",
        &[&scope.tenant_id, &scope.user_id, &scope.agent_id, &scope.project_id,
          &instruction.recipe().uuid, &instruction.variant().step_link],
    ).await.unwrap();
    assert!(changed > 0);
    assert!(
        matches!(
            verify_routing_metadata(&routing_tx, scope, &[instruction]).await,
            Err(McpDiscoveryError::Unqualified)
        ),
        "corrupt physical routing anchors must fail closed"
    );
    routing_tx.rollback().await.unwrap();
    assert!(
        matches!(
            qualify_installed_commands(pool, scope, generation, &[selected], &[selected], None)
                .await,
            Err(McpDiscoveryError::Unqualified)
        ),
        "observations without trusted activation approval cannot qualify advertising"
    );
    let row = client.query_one(
        "SELECT checksum,evidence_bytes FROM brassclaw_mcp_command_qualifications WHERE catalogue_id=$1 ORDER BY checksum LIMIT 1",
        &[&generation],
    ).await.unwrap();
    let checksum: String = row.get(0);
    let bytes: String = row.get(1);
    assert_eq!(digest(&bytes), checksum);
    let proof: Value = serde_json::from_str(&bytes).unwrap();
    let command = &proof["commands"]["publish_literal_reply"];
    assert_eq!(command["successes"].as_array().unwrap().len(), 3);
    assert_eq!(command["failures"].as_array().unwrap().len(), 1);
    assert_eq!(command["negative_commands"].as_array().unwrap().len(), 2);
    assert_eq!(proof["semantic_approval"], false);
    assert_eq!(proof["catalogue_activation"], false);
    assert!(client.execute("UPDATE brassclaw_mcp_command_qualifications SET evidence_bytes=$1 WHERE checksum=$2", &[&bytes, &checksum]).await.is_err());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualified_commands_reject_potential_overlap_even_without_example_collision() {
        assert!(may_overlap("read %", "read file %"));
        assert!(may_overlap("read %; interval %", "read %; interval 1:4"));
        assert!(may_overlap("% suffix", "long % suffix"));
        assert!(!may_overlap("read %", "write %"));
        assert!(!may_overlap("% as text", "% as json"));
        assert!(
            serde_json::from_value::<CommandCases>(
                json!({"format":"mcp-command-cases/1","failure_examples":[],"approved":true})
            )
            .is_err()
        );
    }
}
