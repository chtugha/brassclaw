//! Read-only initial bootstrap packet DATA from actual original model/setup
//! owners. No Use/job admission, discovery, model dispatch, interpretation,
//! prefill import, approval or Serving authority is created here.
use super::{Error, PROPOSAL_LIMITS, PgPrefixModelDesigns, bad, sha};
use brassclaw_product_workflow::{
    PrefixModelSetupDraft, PrefixModelSetupOrigin, PrefixModelSpecification,
};
use brassclaw_skills::{
    prefix_bootstrap_components::RESPONSE_SCHEMA,
    value_contract::{ContractLimits, strict_json, validate_data_bounds},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(crate) enum BootstrapPacketTask {
    ExtractFacts,
}
impl BootstrapPacketTask {
    fn name(self) -> &'static str {
        match self {
            Self::ExtractFacts => "extract_bootstrap_facts",
        }
    }
}

/// Original native caller supplies exact references, never today's role or a
/// model echo. These are data addresses, NOT an admission/qualification token.
pub(crate) struct BootstrapPacketInput<'a> {
    pub(crate) request_id: &'a str,
    pub(crate) target_specification: &'a str,
    pub(crate) executing_specification: &'a str,
    pub(crate) setup_revision: u64,
    pub(crate) task: BootstrapPacketTask,
    /// Exact Recipe-selected list of {field,scope}, not LLM-generated needs.
    pub(crate) required_facts: &'a Value,
}

/// No Deserialize/public success flag. The bytes are protocol data only; this
/// wrapper cannot authorize a model call or prove original Use/job admission.
pub(crate) struct BootstrapPacketData {
    bytes: String,
}
impl BootstrapPacketData {
    pub(crate) fn packet_bytes(&self) -> &str {
        &self.bytes
    }
}

fn pair(specification: &PrefixModelSpecification) -> Value {
    json!({"specification":specification.id,"provider_id":specification.provider_id,
        "provider_revision":specification.provider_revision,"model":specification.model})
}

impl PgPrefixModelDesigns {
    /// Data assembly over actual owner-verified original records. The native
    /// transaction owner may use these same records without a pool re-read.
    /// Public DTO shape alone is not proof of provenance or execution authority.
    pub(crate) fn assemble_bootstrap_packet(
        input: BootstrapPacketInput<'_>,
        target: &PrefixModelSpecification,
        executing: &PrefixModelSpecification,
        setup: &PrefixModelSetupDraft,
        limits: ContractLimits,
    ) -> Result<BootstrapPacketData, Error> {
        if target.id != input.target_specification
            || executing.id != input.executing_specification
            || setup.specification != target.id
            || setup.revision != input.setup_revision
        {
            return Err(Error::Conflict);
        }
        if input.request_id.is_empty()
            || input.request_id.len() > limits.max_bytes
            || input.request_id.trim() != input.request_id
            || input.request_id.chars().any(char::is_control)
        {
            return Err(bad());
        }
        validate_data_bounds(input.required_facts, limits).map_err(|_| bad())?;
        let schema = strict_json(
            RESPONSE_SCHEMA,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .map_err(|_| Error::Unavailable)?;
        let enums = &schema["$defs"]["fact"]["properties"];
        let fields = enums["field"]["enum"]
            .as_array()
            .ok_or(Error::Unavailable)?;
        let scopes = enums["scope"]["enum"]
            .as_array()
            .ok_or(Error::Unavailable)?;
        let requirements = input.required_facts.as_array().ok_or_else(bad)?;
        let mut seen = BTreeSet::new();
        for requirement in requirements {
            if requirement
                .as_object()
                .is_none_or(|object| object.len() != 2)
                || !fields.contains(&requirement["field"])
                || !scopes.contains(&requirement["scope"])
                || !seen.insert((requirement["field"].as_str(), requirement["scope"].as_str()))
            {
                return Err(bad());
            }
        }
        // This bounded DATA producer represents the exact unaccepted setup
        // revision only, not active generation values. Current Accepted/1 does
        // not bind that budget revision. Never look up today's active state or
        // invent those missing original values; B2 emits no eligible options.
        // A later actual import still requires original active-value capture.
        let original = strict_json(&target.specification_bytes, PROPOSAL_LIMITS)
            .map_err(|_| Error::Unavailable)?;
        // Lossless explicit field excerpt; arbitrary unknown stored fields can
        // never smuggle credentials into this reusable packet. Full original
        // bytes remain in the original specification owner at source_id/hash.
        let paths = [
            "format",
            "provider_namespace",
            "provider_id",
            "provider_revision",
            "model",
            "protocol",
            "configured_context_window",
            "context_evidence",
            "model_token_limits",
            "model_token_limits_owner",
            "qualification",
        ];
        let mut view = serde_json::Map::new();
        let mut pointers = Vec::new();
        for name in paths {
            if let Some(value) = original.get(name) {
                view.insert(name.into(), value.clone());
                pointers.push(format!("/{name}"));
            }
        }
        let source_id = format!("prefix-model-specification:{}", target.id);
        let source = json!({"source_id":source_id,"sha256":sha(target.specification_bytes.as_bytes()),
            "origin":{"kind":"retained-provider-configuration","specification":target.id,"provider_revision":target.provider_revision},
            "target":pair(target),"scope":"provider_configuration","format":"json",
            "qualification":"configured-model-contract-only","projection":pointers,"json":view});
        let mut existing = serde_json::Map::new();
        let owner_revision = format!(
            "prefix-model-setup:{}:revision:{}",
            target.id, setup.revision
        );
        for (field, previous) in &setup.fields {
            let field = serde_json::to_value(field).map_err(|_| Error::Unavailable)?;
            existing.insert(field.as_str().ok_or(Error::Unavailable)?.into(), json!({
                "value":previous.value,"owner_revision":owner_revision,
                "protection":if previous.origin==PrefixModelSetupOrigin::Manual {"preserve"} else {"replaceable"}
            }));
        }
        // Known configuration stays in its actual source/existing-values scope.
        // It cannot become independently verified endpoint/model-serving facts.
        // There are currently no qualified endpoint observation/rule inputs to
        // this bounded producer: represent each such missing fact explicitly.
        let mut observations = Vec::new();
        for field in fields {
            for scope in scopes {
                observations.push(json!({"field":field,"scope":scope,"status":"unknown",
                    "value":null,"evidence_ref":source_id}));
            }
        }
        let payload = json!({"task":input.task.name(),"target":pair(target),"executing_pair":pair(executing),
            "required_facts":input.required_facts,"sources":[source],"checked_observations":observations,
            "eligible_options":[],"existing_values":existing,"response_contract":schema});
        validate_data_bounds(&payload, limits).map_err(|_| bad())?;
        let payload_bytes = crate::prefix_generations::canonical(&payload)?;
        let packet = json!({"request_id":input.request_id,"input_sha256":sha(payload_bytes.as_bytes()),"payload":payload});
        validate_data_bounds(&packet, limits).map_err(|_| bad())?;
        let bytes = crate::prefix_generations::canonical(&packet)?;
        if bytes.len() > limits.max_bytes {
            return Err(bad());
        }
        Ok(BootstrapPacketData { bytes })
    }
}
