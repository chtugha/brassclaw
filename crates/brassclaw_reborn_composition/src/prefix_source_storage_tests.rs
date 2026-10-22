//! Native identity/typed preload preparation. Storage behavior is separately
//! exercised against real PostgreSQL in pg_prefix_source_backend::tests.
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_skills::{
    component_revision::RetainedComponentSnapshot, global_bootstrap_components::UsageComponentIds,
    prefix_source_components::*,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[tokio::test]
async fn source_storage_usages_prepare_shared_native_identity_with_separate_canonical_recipes() {
    let native = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let capability =
        brassclaw_host_api::CapabilityId::new(brassclaw_host_runtime::PREFIX_SOURCE_CAPABILITY_ID)
            .unwrap();
    let identity = native.native_identity(&capability).unwrap();
    let tool = Uuid::new_v4();
    for (usage, operation) in [
        (SourceUsage::Read, "read"),
        (SourceUsage::RetainObservation, "retain_observation"),
    ] {
        let ids = UsageComponentIds {
            tool,
            recipe: Uuid::new_v4(),
            python_code: Uuid::new_v4(),
            tool_skill: Uuid::new_v4(),
            skill: Uuid::new_v4(),
            formatter: Some(Uuid::new_v4()),
        };
        let mut drafts = source_usage_drafts(ids, usage).unwrap();
        drafts.push(source_tool_draft(tool, serde_json::to_value(identity).unwrap()).unwrap());
        let retained = RetainedComponentSnapshot::new(
            &[ids.recipe],
            drafts
                .into_iter()
                .map(|d| d.at_version(1).unwrap())
                .collect(),
        )
        .unwrap();
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(retained),
                ids.recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(program.bindings().len(), 1);
        let binding = &program.bindings()["0:3"];
        assert_eq!(binding.tool().uuid, tool);
        assert_eq!(binding.capability_id(), capability.as_str());
        let mut inputs = json!({"source":{"profile":"sempai","source_id":Uuid::new_v4(),"revision":1,"registration_sha256":"a".repeat(64)}});
        if operation == "retain_observation" {
            inputs["diagnostic_bytes"] = json!("{\"literal\":\"'; host.forbidden() #\"}");
        }
        let mut arguments = inputs.clone();
        arguments["operation"] = json!(operation);
        assert_eq!(
            binding.bind_tool_arguments(&inputs, &arguments).unwrap(),
            arguments
        );
        let mut missing = inputs.clone();
        missing["source"]
            .as_object_mut()
            .unwrap()
            .remove("revision");
        assert!(binding.bind_tool_arguments(&missing, &arguments).is_err());
        let mut extra = inputs.clone();
        extra["activate"] = json!(true);
        assert!(binding.bind_tool_arguments(&extra, &arguments).is_err());
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(Arc::new(program)),
            &brassclaw_monty_host::process::installed_worker().unwrap(),
        )
        .await
        .unwrap();
    }
}
