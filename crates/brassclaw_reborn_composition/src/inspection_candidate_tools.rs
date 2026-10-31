//! Closed development observers reuse the ordinary dispatcher and journal.
//! These constructors grant no qualification or production catalogue selection.
use super::*;

// Closed design-selection execution reuses the two existing primitives. The
// captured variant hashes its authoritative originals before checking coverage.
pub(crate) fn design_selection_candidate_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    let expected: &[(&str, &str)] = if program.bindings().contains_key("2:4") {
        &[
            ("2:4", brassclaw_host_runtime::JSON_CAPABILITY_ID),
            ("2:9", brassclaw_host_runtime::JSON_CAPABILITY_ID),
            ("2:11", brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID),
        ]
    } else if program.bindings().contains_key("1:3") {
        &[
            ("1:3", brassclaw_host_runtime::JSON_CAPABILITY_ID),
            ("1:8", brassclaw_host_runtime::JSON_CAPABILITY_ID),
            ("1:10", brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID),
        ]
    } else {
        &[
            ("0:3", brassclaw_host_runtime::JSON_CAPABILITY_ID),
            ("0:5", brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID),
        ]
    };
    if program.bindings().len() != expected.len()
        || expected.iter().any(|(step, capability)| {
            program
                .bindings()
                .get(*step)
                .is_none_or(|binding| binding.capability_id() != *capability)
        })
    {
        return Err(invalid("design selection candidate bindings differ"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, false)
}

pub(crate) fn source_inventory_candidate_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    let bindings = program.bindings();
    let optional_reply = bindings.len() == 2
        && bindings
            .get("0:5")
            .is_some_and(|binding| binding.capability_id() == "host.post_reply");
    if (bindings.len() != 1 && !optional_reply)
        || bindings.get("0:3").is_none_or(|binding| {
            binding.capability_id() != brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID
        })
    {
        return Err(invalid("source inventory candidate binding differs"));
    }
    let mut runtimes = BTreeMap::new();
    for (step, binding) in program.bindings() {
        let capability = CapabilityId::new(binding.capability_id())
            .map_err(|error| invalid(error.to_string()))?;
        let reply = capability.as_str() == "host.post_reply";
        let descriptor = if reply {
            crate::monty_kernel::reply_package()?
                .capabilities
                .into_iter()
                .next()
                .ok_or_else(|| invalid("inventory reply descriptor missing"))?
        } else {
            kernel
                .builtin(&capability, None)
                .map_err(|error| invalid(error.to_string()))?
                .descriptor()
                .clone()
        };
        let policy = Arc::new(PgSelectedToolPolicy {
            host: host.clone(),
            pool: pool.clone(),
            tool: binding.tool().uuid,
            capability: capability.clone(),
            effects: descriptor.effects,
            mounts: MountView::default(),
        });
        let runtime = if reply {
            kernel.reply(host.clone(), policy)?
        } else {
            let document = program.inputs().instruction().snapshot().revisions()
                [&binding.tool().uuid]
                .draft()
                .document();
            let expected = serde_json::from_value(document["implementation"]["identity"].clone())
                .map_err(|error| invalid(error.to_string()))?;
            kernel
                .builtin_with_policy(&capability, expected, policy)
                .map_err(|error| invalid(error.to_string()))?
        };
        runtimes.insert(step.clone(), Arc::new(runtime));
    }
    Ok(Arc::new(SelectedTools {
        host,
        admission,
        program,
        runtimes,
        kernel,
        credential_policies: BTreeMap::new(),
        credentials: crate::task_tool_credentials::CapturedToolCredentials::Uncaptured,
        mounts: MountView::default(),
        answers: native_answers::NativeToolAnswers::default(),
        model_responses: None,
    }))
}

pub(crate) fn response_parser_candidate_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.bindings().len() != 1
        || program.bindings().get("0:2").is_none_or(|binding| {
            binding.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID
        })
    {
        return Err(invalid("response parser candidate binding differs"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, false)
}

// Closed development caller only. No workflow qualification or public admission.
pub(crate) fn pair_case_candidate_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.bindings().len() != 2
        || program.bindings().get("0:3").is_none_or(|binding| {
            binding.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID
        })
        || program.bindings().get("0:5").is_none_or(|binding| {
            binding.capability_id() != brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID
        })
    {
        return Err(invalid("pair case candidate bindings differ"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, false)
}
