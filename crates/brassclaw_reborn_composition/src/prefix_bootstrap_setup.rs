//! Native capture/result boundary of the single packaged technical Recipe.
//! It neither discovers endpoints nor accepts a design or serving generation.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::executor::retained_recipe::{
    RetainedRecipeExecution, typed_value_checksum_with_limits,
};
use brassclaw_product_workflow::{PrefixModelSetupDraft, PrefixModelSpecification};
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde_json::{Value, json};
use uuid::Uuid;

/// Construct only from records verified by the existing native specification /
/// setup owner. Caller holds the same specification lock as manual writes.
pub(crate) fn inputs(
    job: Uuid,
    origin: Uuid,
    specification: &PrefixModelSpecification,
    setup: &PrefixModelSetupDraft,
    limits: ContractLimits,
) -> Result<Value, RebornBuildError> {
    if job.is_nil() || origin.is_nil() {
        return Err(invalid("bootstrap identities missing"));
    }
    let packet = crate::prefix_model_designs::PgPrefixModelDesigns::assemble_bootstrap_packet(
        crate::prefix_model_designs::BootstrapPacketInput {
            request_id: &job.to_string(),
            target_specification: &specification.id,
            executing_specification: &specification.id,
            setup_revision: setup.revision,
            task: crate::prefix_model_designs::BootstrapPacketTask::ExtractFacts,
            required_facts: &json!([]),
        },
        specification,
        specification,
        setup,
        limits,
    )
    .map_err(invalid)?;
    let packet = strict_json(packet.packet_bytes(), limits).map_err(invalid)?;
    let capture = json!({"format":"brassclaw-bootstrap-setup-capture/1","job_id":job,"origin_mutation":origin,"packet":packet,"setup":setup});
    brassclaw_skills::value_contract::validate_data_bounds(&capture, limits).map_err(invalid)?;
    let inputs =
        json!({"capture_bytes":crate::prefix_generations::canonical(&capture).map_err(invalid)?});
    brassclaw_skills::value_contract::validate_data_bounds(&inputs, limits).map_err(invalid)?;
    Ok(inputs)
}

/// Independent native oracle over typed captured data and the COMPLETE Recipe
/// result. A model string/shape, success flag or source label cannot grant proof.
pub(crate) fn verify_result(
    inputs: &Value,
    receipt: &str,
    limits: ContractLimits,
) -> Result<Value, RebornBuildError> {
    let capture = strict_json(
        inputs["capture_bytes"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap input missing"))?,
        limits,
    )
    .map_err(invalid)?;
    let result = strict_json(receipt, limits).map_err(invalid)?;
    let payload = &capture["packet"]["payload"];
    if result.as_object().is_none_or(|v| v.len() != 9)
        || result["format"] != "brassclaw-bootstrap-setup-result/1"
        || result["job_id"] != capture["job_id"]
        || result["origin_mutation"] != capture["origin_mutation"]
        || result["target"] != payload["target"]
        || result["executing_pair"] != payload["executing_pair"]
        || result["setup"] != capture["setup"]
        || result["configuration"] != payload["sources"][0]
        || result["checked_observations"] != payload["checked_observations"]
        || result["readiness"]
            != json!({"scope":"configuration-prefill-only","generate":"blocked","missing":["qualified-endpoint-discovery","independent-design-acceptance-binding"],"design_acceptance":"unknown","serving":"unmeasured"})
        || result.get("approval").is_some()
        || result.get("active").is_some()
    {
        return Err(invalid(
            "technical bootstrap result differs from original native capture",
        ));
    }
    let setup: PrefixModelSetupDraft =
        serde_json::from_value(result["setup"].clone()).map_err(invalid)?;
    if setup.fields.len() != brassclaw_product_workflow::PrefixModelSetupField::ALL.len()
        || setup
            .fields
            .iter()
            .any(|(field, value)| !field.valid_value(value.value))
    {
        return Err(invalid("bootstrap setup values differ"));
    }
    Ok(result)
}

pub(crate) fn verify_execution(
    inputs: &Value,
    receipt: &str,
    execution: &RetainedRecipeExecution,
) -> Result<Value, RebornBuildError> {
    let order = vec!["0:1".to_owned()];
    let proof = execution
        .observations()
        .get("0:1")
        .ok_or_else(|| invalid("bootstrap native step proof missing"))?;
    let result = verify_result(inputs, receipt, proof.data_limits())?;
    if !execution.is_complete()
        || execution.transport_failure().is_some()
        || !execution.host_answers().is_empty()
        || execution.observation_order() != Some(order.as_slice())
        || execution.observations().len() != 1
        || !proof.settled()
        || proof.failure().is_some()
        || proof.observation_error().is_some()
        || proof.arguments_checksum().is_some()
        || proof.answer_checksum().is_some()
        || proof.input_checksum()
            != typed_value_checksum_with_limits(inputs, proof.data_limits()).map_err(invalid)?
        || proof.result_checksum()
            != Some(
                typed_value_checksum_with_limits(&json!(receipt), proof.data_limits())
                    .map_err(invalid)?,
            )
    {
        return Err(invalid("bootstrap actual pure execution differs"));
    }
    Ok(result)
}

/// Revalidate originals and exact qualification before selecting the shared
/// internal ResultOnly runner. No current role, default prefix or model fallback.
pub(crate) async fn select(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    host: std::sync::Arc<brassclaw_reborn::monty_task_host::MontyTaskHost>,
    admission: std::sync::Arc<crate::pg_monty_admission::PgMontyAdmission>,
    definition: crate::prefix_job_definitions::Definition,
    reference: String,
) -> Result<std::sync::Arc<dyn crate::global_recipe_ports::InternalRecipeSource>, RebornBuildError>
{
    use brassclaw_engine::executor::retained_recipe::RetainedProgram;
    let crate::prefix_job_definitions::Target::ModelSetup {
        specification,
        setup_revision,
    } = &definition.target
    else {
        return Err(invalid("setup target missing"));
    };
    let inspected = catalogue.setup_qualification.program();
    if inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?
        .exact_bytes()
        != definition.program_bytes
    {
        return Err(invalid("setup retained program differs"));
    }
    let mut client = catalogue.pool.get().await.map_err(invalid)?;
    let tx = client.transaction().await.map_err(invalid)?;
    catalogue
        .setup_qualification
        .verify_in_transaction(&tx)
        .await?;
    let origin: Uuid =
        serde_json::from_value(definition.request["origin_mutation"].clone()).map_err(invalid)?;
    let mutation =
        crate::provider_mutation::read_original(&*tx, definition.scope.tenant_id.as_str(), origin)
            .await
            .map_err(invalid)?;
    if mutation.bootstrap_job_id != Some(definition.id) {
        return Err(invalid("setup original mutation link differs"));
    }
    let store = crate::prefix_model_designs::PgPrefixModelDesigns::new(
        catalogue.pool.clone(),
        definition.scope.tenant_id.as_str().into(),
    );
    let original = store
        .specification_in_transaction(&tx, specification)
        .await
        .map_err(invalid)?;
    let setup = store
        .retained_setup_in_transaction(&tx, specification, *setup_revision)
        .await
        .map_err(invalid)?;
    if inputs(
        definition.id,
        origin,
        &original,
        &setup,
        definition.original_input_limits,
    )? != definition.input
    {
        return Err(invalid("setup original input differs"));
    }
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("setup pure program missing"));
    };
    let program = program.clone();
    let pool = catalogue.pool.clone();
    let kernel = catalogue.kernel.clone();
    let bind_tools = Box::new(move || {
        crate::installed_monty_catalogue::bootstrap_setup_tools(
            pool.clone(),
            kernel.clone(),
            host.clone(),
            admission.clone(),
            program.clone(),
        )
    });
    Ok(std::sync::Arc::new(
        crate::prefix_job_execution::Invocation {
            receipt_contract: crate::prefix_job_execution::ReceiptContract::ModelSetup,
            definition,
            reference,
            inspected,
            tools: Default::default(),
            bind_tools,
            pool: catalogue.pool.clone(),
        },
    ))
}
