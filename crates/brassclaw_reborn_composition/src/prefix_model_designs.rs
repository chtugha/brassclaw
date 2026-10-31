//! Retain model-contract and declarative design proposals using existing provider
//! identities. Unknown artifacts/cache facts stay Unknown. No provider dispatch,
//! design acceptance, generation selection or activation occurs in this store.
use crate::pg_provider_repo::PgProviderRepo;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    CapturePrefixModelRequest, InterceptorConfigServiceError as Error, PrefixDesignProposal,
    PrefixModelDesignCatalogue, PrefixModelSpecification, PrefixModelTarget, PrefixPairCasePacket,
    PrefixPairConstructionProposal, RetainPrefixDesignProposalRequest,
};
use brassclaw_skills::{
    prefix_generation_rules::PrefixGenerationRules,
    value_contract::{ContractLimits, strict_json},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
pub(crate) const PROPOSAL_LIMITS: ContractLimits = ContractLimits {
    max_depth: 32,
    max_nodes: 32768,
    max_bytes: 1048576,
};
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub(crate) fn bad() -> Error {
    Error::InvalidRequest {
        reason: "invalid model/design reference or unsupported design options".into(),
    }
}
pub(crate) fn hash(value: &str) -> Result<(), Error> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad());
    }
    Ok(())
}
/// Transient build reservations, not another provider/profile setting.
#[derive(Clone, Copy)]
pub(crate) struct PrefixBuildReservations {
    pub(crate) requested_prefix_tokens: u64,
    pub(crate) reserved_output_tokens: u64,
    pub(crate) additional_framing_tokens: u64,
}

#[path = "prefix_bootstrap_packet.rs"]
mod bootstrap_packet;
#[path = "prefix_model_setup.rs"]
mod setup;
pub(crate) use bootstrap_packet::{BootstrapPacketInput, BootstrapPacketTask};

pub(crate) struct PgPrefixModelDesigns {
    pool: Arc<PgPool>,
    namespace: String,
}
impl PgPrefixModelDesigns {
    pub(crate) fn new(pool: Arc<PgPool>, namespace: String) -> Self {
        Self { pool, namespace }
    }
    // A self-consistent snapshot hash does not establish the origin of its
    // configuration facts. Use the existing immutable provider owner, never
    // today's limits or a second editable prefix policy.
    async fn verified_specifications<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
        rows: &[tokio_postgres::Row],
    ) -> Result<Vec<PrefixModelSpecification>, Error> {
        let repo = PgProviderRepo::new((*self.pool).clone(), self.namespace.clone());
        let mut originals = BTreeMap::new();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let specification = specification(row)?;
            let key = (
                specification.provider_id.clone(),
                specification.provider_revision,
            );
            // Several model snapshots can share one provider revision.
            if let std::collections::btree_map::Entry::Vacant(entry) = originals.entry(key.clone())
            {
                let original = repo
                    .read_retained_revision(client, &key.0, key.1)
                    .await
                    .map_err(|_| Error::Unavailable)?
                    .ok_or(Error::Unavailable)?;
                entry.insert(original);
            }
            let original = &originals[&key].definition;
            let value = strict_json(&specification.specification_bytes, PROPOSAL_LIMITS)
                .map_err(|_| Error::Unavailable)?;
            let context = (specification.model == original.default_model)
                .then_some(original.context_window_tokens)
                .flatten();
            if value["provider_namespace"] != self.namespace
                || value["protocol"] != json!(&original.protocol)
                || value["context_evidence"] != "provider-configuration-only"
                || value["configured_context_window"] != json!(context)
            {
                return Err(Error::Unavailable);
            }
            match (
                value.get("model_token_limits"),
                value.get("model_token_limits_owner"),
            ) {
                // Earlier immutable snapshots did not record these facts.
                // Preserve Unknown; never backfill from a later revision.
                (None, None) => {}
                (Some(limits), Some(owner))
                    if owner == "provider-definition-revision"
                        && *limits
                            == json!(original.model_token_limits.0.get(&specification.model)) => {}
                _ => return Err(Error::Unavailable),
            }
            result.push(specification);
        }
        Ok(result)
    }
    pub(crate) async fn catalogue(
        &self,
        targets: Vec<PrefixModelTarget>,
    ) -> Result<PrefixModelDesignCatalogue, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let rows=client.query("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE provider_namespace=$1 ORDER BY created_at,specification_id",&[&self.namespace]).await.map_err(|_|Error::Unavailable)?;
        let specifications = self.verified_specifications(&**client, &rows).await?;
        let proposals=client.query("SELECT p.proposal_id,p.profile_name,p.specification_id,p.proposal_bytes FROM reborn_prefix_design_proposals p JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE s.provider_namespace=$1 AND p.profile_name IS NOT NULL ORDER BY p.created_at,p.proposal_id",&[&self.namespace]).await.map_err(|_|Error::Unavailable)?.iter().map(proposal).collect::<Result<Vec<_>,_>>()?;
        let pair_proposals=client.query("SELECT p.proposal_id,p.specification_id,p.proposal_bytes FROM reborn_prefix_design_proposals p JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE s.provider_namespace=$1 AND p.profile_name IS NULL ORDER BY p.created_at,p.proposal_id",&[&self.namespace]).await.map_err(|_|Error::Unavailable)?.iter().map(pair_proposal).collect::<Result<Vec<_>,_>>()?;
        drop(client);
        Ok(PrefixModelDesignCatalogue {
            targets,
            specifications,
            proposals,
            pair_proposals,
            artifact_sets: crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
                self.pool.clone(),
                self.namespace.clone(),
            )
            .list()
            .await?,
            design_generation_available: false,
            blocked_reason: Some("prefix.modelDesignPrerequisites".into()),
        })
    }
    pub(crate) async fn capture(
        &self,
        request: CapturePrefixModelRequest,
        targets: Vec<PrefixModelTarget>,
    ) -> Result<PrefixModelSpecification, Error> {
        if request.model.is_empty() || request.model.chars().count() > 2048 {
            return Err(bad());
        }
        let requested_revision =
            i64::try_from(request.expected_provider_revision).map_err(|_| bad())?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let previous = client.query("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE provider_namespace=$1 AND provider_id=$2 AND provider_revision=$3 AND model=$4", &[&self.namespace,&request.provider_id,&requested_revision,&request.model]).await.map_err(|_| Error::Unavailable)?;
        if previous.len() > 1 {
            return Err(Error::Conflict);
        }
        if !previous.is_empty() {
            // A lost-response retry returns the exact historical contract, even
            // after selection changes. It neither captures nor qualifies the new target.
            return self
                .verified_specifications(&**client, &previous)
                .await?
                .pop()
                .ok_or(Error::Unavailable);
        }
        drop(client);
        if !targets.iter().any(|target| {
            target.provider_id == request.provider_id
                && target.provider_revision == request.expected_provider_revision
                && target.model == request.model
        }) {
            return Err(Error::Conflict);
        }
        let repo = PgProviderRepo::new((*self.pool).clone(), self.namespace.clone());
        let retained = repo
            .get_revision(&request.provider_id, false)
            .await
            .map_err(|_| Error::Unavailable)?
            .ok_or(Error::Conflict)?;
        if retained.revision != request.expected_provider_revision {
            return Err(Error::Conflict);
        }
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        self.capture_original(&**client, &retained, &request.model)
            .await
    }

    /// Only a real owner-retained provider revision may enter this historical
    /// writer. Shared caller transaction owns all selection/spec/setup effects.
    pub(crate) async fn capture_original<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
        retained: &crate::pg_provider_repo::ProviderDefinitionRevision,
        model: &str,
    ) -> Result<PrefixModelSpecification, Error> {
        if model.is_empty() || model.chars().count() > 2048 {
            return Err(bad());
        }
        let original = PgProviderRepo::new((*self.pool).clone(), self.namespace.clone())
            .read_retained_revision(client, &retained.definition.id, retained.revision)
            .await
            .map_err(|_| Error::Unavailable)?
            .ok_or(Error::Conflict)?;
        if original.revision != retained.revision
            || serde_json::to_value(&original.definition).map_err(|_| Error::Unavailable)?
                != serde_json::to_value(&retained.definition).map_err(|_| Error::Unavailable)?
        {
            return Err(Error::Conflict);
        }
        let revision = i64::try_from(retained.revision).map_err(|_| bad())?;
        let context = if model == retained.definition.default_model {
            retained.definition.context_window_tokens
        } else {
            None
        };
        let bytes=json!({"format":"brassclaw-prefix-model-specification/1","provider_namespace":self.namespace,"provider_id":retained.definition.id,"provider_revision":revision,"model":model,"protocol":retained.definition.protocol,
            "configured_context_window":context,"context_evidence":"provider-configuration-only",
            "model_token_limits":retained.definition.model_token_limits.0.get(model),
            "model_token_limits_owner":"provider-definition-revision",
            "weights_identity":null,"config_identity":null,"tokenizer_identity":null,"template_identity":null,
            "architecture":null,"attention_recurrent_state":null,"runtime_cache":null,
            "qualification":"configured-model-contract-only"}).to_string();
        let id = sha(bytes.as_bytes());
        client.execute("INSERT INTO reborn_prefix_model_specifications(specification_id,provider_namespace,provider_id,provider_revision,model,specification_bytes) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",&[&id,&self.namespace,&retained.definition.id,&revision,&model,&bytes]).await.map_err(|_|Error::Unavailable)?;
        let row=client.query_one("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE specification_id=$1",&[&id]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, String>(4) != bytes {
            return Err(Error::Conflict);
        }
        self.verified_specifications(client, std::slice::from_ref(&row))
            .await?
            .pop()
            .ok_or(Error::Unavailable)
    }
    pub(crate) async fn get_specification(
        &self,
        id: &str,
    ) -> Result<PrefixModelSpecification, Error> {
        self.read_model_input(id, None)
            .await
            .map(|(specification, _)| specification)
    }
    pub(crate) async fn get_proposal(
        &self,
        profile: &str,
        id: &str,
    ) -> Result<PrefixDesignProposal, Error> {
        hash(id)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row = client.query_opt("SELECT p.proposal_id,p.profile_name,p.specification_id,p.proposal_bytes FROM reborn_prefix_design_proposals p JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE p.proposal_id=$1 AND p.profile_name=$2 AND s.provider_namespace=$3", &[&id, &profile, &self.namespace]).await.map_err(|_| Error::Unavailable)?.ok_or(Error::Conflict)?;
        proposal(&row)
    }

    /// Resolve immutable canonical configuration for the existing pure planner.
    /// Requested output/framing/size are explicit build data, never new settings.
    /// Live admission/final dispatch must independently fence a changed policy.
    pub(crate) async fn read_model_input(
        &self,
        specification: &str,
        reservations: Option<PrefixBuildReservations>,
    ) -> Result<(PrefixModelSpecification, Option<Value>), Error> {
        hash(specification)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row = client.query_opt("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE specification_id=$1 AND provider_namespace=$2", &[&specification, &self.namespace]).await.map_err(|_| Error::Unavailable)?.ok_or(Error::Conflict)?;
        let retained = self
            .verified_specifications(&**client, std::slice::from_ref(&row))
            .await?
            .pop()
            .ok_or(Error::Unavailable)?;
        let Some(reservations) = reservations else {
            return Ok((retained, None));
        };
        let missing = || Error::InvalidRequest {
            reason: "recorded model limits/context and explicit build reservations are required"
                .into(),
        };
        let original = strict_json(&retained.specification_bytes, PROPOSAL_LIMITS)
            .map_err(|_| Error::Unavailable)?;
        let limits: brassclaw_llm::ModelTokenLimits =
            serde_json::from_value(original["model_token_limits"].clone())
                .map_err(|_| missing())?;
        limits.validate().map_err(|_| missing())?;
        let inputs = json!({
            "policy_ref":format!("prefix-model-specification:{}", retained.id),
            "max_prefix_tokens":limits.max_prefix_tokens,
            "max_prompt_tokens":limits.max_prompt_tokens,
            "context_window_tokens":original["configured_context_window"],
            "requested_prefix_tokens":reservations.requested_prefix_tokens,
            "reserved_output_tokens":reservations.reserved_output_tokens,
            "additional_framing_tokens":reservations.additional_framing_tokens,
        });
        // Reuse the actual planner's recursive contract, not a second set of
        // integer bounds/defaults or a Rust copy of its budget arithmetic.
        let planner = brassclaw_skills::prefix_compiler_components::model_budget_draft(
            crate::installed_monty_catalogue::id("prefix-design-selection:model-budget"),
        )
        .map_err(|_| Error::Unavailable)?;
        brassclaw_skills::value_contract::InputContract::from_value(
            &planner.document()["input_contract"],
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .map_err(|_| Error::Unavailable)?
        .bind(&inputs)
        .map_err(|_| missing())?;
        Ok((retained, Some(inputs)))
    }
    pub(crate) async fn get_pair_proposal(
        &self,
        specification: &str,
        proposal_id: &str,
    ) -> Result<PrefixPairConstructionProposal, Error> {
        hash(specification)?;
        hash(proposal_id)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        self.get_pair_proposal_on(&**client, specification, proposal_id)
            .await
    }
    /// Immutable Pair reference resolution within the caller's coherent read.
    /// Same namespace/hash/parser owner; no new model/design registry.
    pub(crate) async fn get_pair_proposal_on<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
        specification: &str,
        proposal_id: &str,
    ) -> Result<PrefixPairConstructionProposal, Error> {
        hash(specification)?;
        hash(proposal_id)?;
        let row=client.query_opt("SELECT p.proposal_id,p.specification_id,p.proposal_bytes FROM reborn_prefix_design_proposals p JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE p.proposal_id=$1 AND p.specification_id=$2 AND p.profile_name IS NULL AND s.provider_namespace=$3",&[&proposal_id,&specification,&self.namespace]).await.map_err(|_|Error::Unavailable)?.ok_or(Error::Conflict)?;
        pair_proposal(&row)
    }
    pub(crate) async fn case_packet(
        &self,
        specification: &str,
        proposal_id: &str,
    ) -> Result<PrefixPairCasePacket, Error> {
        let retained = self.get_pair_proposal(specification, proposal_id).await?;
        let document =
            crate::prefix_pair_construction::parse(specification, &retained.proposal_bytes)
                .map_err(|_| Error::Unavailable)?;
        let detail = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
            self.pool.clone(),
            self.namespace.clone(),
        )
        .get(specification, &document.artifact_set)
        .await?;
        crate::prefix_pair_construction::verify_sources(&document, &detail)?;
        let case_packet_bytes =
            crate::prefix_generations::canonical(&document.case_packet(&retained.id))
                .map_err(|_| Error::Unavailable)?;
        if case_packet_bytes.len() > PROPOSAL_LIMITS.max_bytes {
            return Err(bad());
        }
        Ok(PrefixPairCasePacket {
            proposal: retained.id,
            specification: document.specification,
            artifact_set: document.artifact_set,
            case_packet_bytes,
            qualification: "unqualified-case-input".into(),
        })
    }
    pub(crate) async fn retain_pair_proposal(
        &self,
        request: RetainPrefixDesignProposalRequest,
    ) -> Result<PrefixPairConstructionProposal, Error> {
        let document = crate::prefix_pair_construction::parse(
            &request.specification,
            &request.proposal_bytes,
        )?;
        // Exact namespace-bound originals are checked before any proposal insert.
        let detail = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
            self.pool.clone(),
            self.namespace.clone(),
        )
        .get(&document.specification, &document.artifact_set)
        .await?;
        crate::prefix_pair_construction::verify_sources(&document, &detail)?;
        let id = sha(request.proposal_bytes.as_bytes());
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        client.execute("INSERT INTO reborn_prefix_design_proposals(proposal_id,profile_name,specification_id,proposal_bytes) VALUES($1,NULL,$2,$3) ON CONFLICT DO NOTHING", &[&id,&request.specification,&request.proposal_bytes]).await.map_err(|_|Error::Unavailable)?;
        let row=client.query_one("SELECT p.proposal_id,p.specification_id,p.proposal_bytes FROM reborn_prefix_design_proposals p JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE p.proposal_id=$1 AND p.profile_name IS NULL AND s.provider_namespace=$2", &[&id,&self.namespace]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, String>(2) != request.proposal_bytes {
            return Err(Error::Conflict);
        }
        pair_proposal(&row)
    }
    pub(crate) async fn retain_proposal(
        &self,
        profile: &str,
        request: RetainPrefixDesignProposalRequest,
    ) -> Result<PrefixDesignProposal, Error> {
        let value = validate_proposal(profile, &request.specification, &request.proposal_bytes)?;
        validate_current_compiler_options(profile, &value)?;
        let id = sha(request.proposal_bytes.as_bytes());
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        if client.query_opt("SELECT specification_id FROM reborn_prefix_model_specifications WHERE specification_id=$1 AND provider_namespace=$2",&[&request.specification,&self.namespace]).await.map_err(|_|Error::Unavailable)?.is_none(){return Err(bad());}
        if client
            .query_opt(
                "SELECT name FROM reborn_prefix_profiles WHERE name=$1",
                &[&profile],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .is_none()
        {
            return Err(Error::PrefixNotFound {
                name: profile.into(),
            });
        }
        client.execute("INSERT INTO reborn_prefix_design_proposals(proposal_id,profile_name,specification_id,proposal_bytes) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",&[&id,&profile,&request.specification,&request.proposal_bytes]).await.map_err(|_|Error::Unavailable)?;
        let row=client.query_one("SELECT proposal_id,profile_name,specification_id,proposal_bytes FROM reborn_prefix_design_proposals WHERE proposal_id=$1",&[&id]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, String>(3) != request.proposal_bytes {
            return Err(Error::Conflict);
        }
        proposal(&row)
    }
}
fn validate_proposal(profile: &str, specification: &str, bytes: &str) -> Result<Value, Error> {
    hash(specification)?;
    let value = strict_json(bytes, PROPOSAL_LIMITS).map_err(|_| bad())?;
    let fields = ["format", "profile", "specification", "design", "rationale"];
    let object = value.as_object().ok_or_else(bad)?;
    if object.len() != fields.len()
        || fields.iter().any(|key| !object.contains_key(*key))
        || !matches!(
            value["format"].as_str(),
            Some("brassclaw-prefix-design-proposal/1" | "brassclaw-prefix-design-proposal/2")
        )
        || value["profile"] != profile
        || value["specification"] != specification
        || value["rationale"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
    {
        return Err(bad());
    }
    let design = value["design"].as_object().ok_or_else(bad)?;
    if value["format"] == "brassclaw-prefix-design-proposal/2" {
        // This is a source-independent reference-design declaration. It cannot
        // be consumed by the historical complete-reference measurement path,
        // nor accepted/activated merely by retention.
        if design.len() != 2
            || design.get("adapter").and_then(Value::as_str) != Some("whole-unit-reference/1")
        {
            return Err(bad());
        }
        let rules: PrefixGenerationRules =
            serde_json::from_value(design.get("generation_rules").cloned().ok_or_else(bad)?)
                .map_err(|_| bad())?;
        if !rules.valid_for_profile(profile) {
            return Err(bad());
        }
        return Ok(value);
    }
    if design.len() != 4
        || value["design"]["adapter"] != "ornith-qwen-system/1"
        || !matches!(
            value["design"]["evidence_view"].as_str(),
            Some(
                "literal-reference/v1"
                    | "literal-reference/v2-deduplicated"
                    | "literal-reference/v3-trailing-inventory"
                    | "literal-reference/v4-stable-sharing"
                    | "literal-reference/v5-contextual-sharing"
            )
        )
        || value["design"]["source_selection"] != "complete-mandatory"
        || value["design"]["reserve_tokens"]
            .as_u64()
            .is_none_or(|n| !(8192..=9_007_199_254_740_991).contains(&n))
    {
        return Err(bad());
    }
    Ok(value)
}
// Current producer support is checked on new writes, separately from immutable
// proposal parsing. Older unaccepted proposals remain readable as history.
fn validate_current_compiler_options(profile: &str, value: &Value) -> Result<(), Error> {
    if value["format"] == "brassclaw-prefix-design-proposal/1"
        && profile == "sempai"
        && value["design"]["evidence_view"] != "literal-reference/v5-contextual-sharing"
    {
        return Err(bad());
    }
    Ok(())
}
fn specification(row: &tokio_postgres::Row) -> Result<PrefixModelSpecification, Error> {
    let bytes: String = row.get(4);
    let id: String = row.get(0);
    if sha(bytes.as_bytes()) != id {
        return Err(Error::Unavailable);
    }
    let value: Value = serde_json::from_str(&bytes).map_err(|_| Error::Unavailable)?;
    if value["format"] != "brassclaw-prefix-model-specification/1"
        || value["qualification"] != "configured-model-contract-only"
        || value["provider_id"] != row.get::<_, String>(1)
        || value["provider_revision"] != row.get::<_, i64>(2)
        || value["model"] != row.get::<_, String>(3)
    {
        return Err(Error::Unavailable);
    }
    Ok(PrefixModelSpecification {
        id,
        provider_id: row.get(1),
        provider_revision: u64::try_from(row.get::<_, i64>(2)).map_err(|_| Error::Unavailable)?,
        model: row.get(3),
        specification_bytes: bytes,
        qualification: "unknown-model-artifacts".into(),
    })
}
fn pair_proposal(row: &tokio_postgres::Row) -> Result<PrefixPairConstructionProposal, Error> {
    let id: String = row.get(0);
    let specification: String = row.get(1);
    let bytes: String = row.get(2);
    if sha(bytes.as_bytes()) != id {
        return Err(Error::Unavailable);
    }
    crate::prefix_pair_construction::parse(&specification, &bytes)
        .map_err(|_| Error::Unavailable)?;
    Ok(PrefixPairConstructionProposal {
        id,
        specification,
        proposal_bytes: bytes,
        qualification: "unaccepted-pair-construction-proposal".into(),
    })
}
fn proposal(row: &tokio_postgres::Row) -> Result<PrefixDesignProposal, Error> {
    let bytes: String = row.get(3);
    let id: String = row.get(0);
    if sha(bytes.as_bytes()) != id {
        return Err(Error::Unavailable);
    }
    validate_proposal(&row.get::<_, String>(1), &row.get::<_, String>(2), &bytes)
        .map_err(|_| Error::Unavailable)?;
    Ok(PrefixDesignProposal {
        id,
        profile: row.get(1),
        specification: row.get(2),
        proposal_bytes: bytes,
        qualification: "unaccepted-proposal".into(),
    })
}

#[cfg(test)]
#[path = "prefix_model_policy_tests.rs"]
mod policy_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_versioned_rule_declarations_without_inventing_geometry() {
        let specification = "a".repeat(64);
        let mut value = json!({"format":"brassclaw-prefix-design-proposal/2",
            "profile":"sempai", "specification":specification,
            "rationale":"Unaccepted budget-dependent reference rules",
            "design":{"adapter":"whole-unit-reference/1","generation_rules":{
                "format":"brassclaw-prefix-generation-rules/1","profile":"sempai",
                "header":"Preserve complete contracts and provenance.",
                "required_coverage":[{"path":"components/","match":"prefix","first_lines":[]}],
                "bands":[{"minimum_tokens":0,"maximum_tokens":100000,"groups":[{
                    "id":"contracts","required":true,"priority":0,"dependencies":[],
                    "selectors":[{"path":"components/","match":"prefix","first_lines":[]}]}]}]}}});
        let parsed = validate_proposal("sempai", &specification, &value.to_string()).unwrap();
        validate_current_compiler_options("sempai", &parsed).unwrap();
        assert!(parsed["design"].get("reserve_tokens").is_none());
        let mut missing_adapter = value.clone();
        missing_adapter["design"]
            .as_object_mut()
            .unwrap()
            .remove("adapter");
        missing_adapter["design"]["unknown"] = json!(true);
        assert!(validate_proposal("sempai", &specification, &missing_adapter.to_string()).is_err());
        value["design"]["generation_rules"]["profile"] = json!("homeassistant");
        assert!(validate_proposal("sempai", &specification, &value.to_string()).is_err());
        value["design"]["generation_rules"]["profile"] = json!("sempai");
        value["design"]["reserve_tokens"] = json!(8192);
        assert!(validate_proposal("sempai", &specification, &value.to_string()).is_err());
        value["design"]
            .as_object_mut()
            .unwrap()
            .remove("reserve_tokens");
        value["design"]["adapter"] = json!("ornith-qwen-system/1");
        assert!(validate_proposal("sempai", &specification, &value.to_string()).is_err());
    }

    #[test]
    fn sempai_proposals_require_the_supported_contextual_layout() {
        let specification = "a".repeat(64);
        for view in [
            "literal-reference/v1",
            "literal-reference/v2-deduplicated",
            "literal-reference/v3-trailing-inventory",
            "literal-reference/v4-stable-sharing",
            "literal-reference/v5-contextual-sharing",
        ] {
            for profile in ["base-prompt", "prefix-maker", "sempai"] {
                let bytes =
                    json!({"format":"brassclaw-prefix-design-proposal/1", "profile":profile,
                    "specification":specification,
                    "design":{"adapter":"ornith-qwen-system/1", "evidence_view":view,
                        "source_selection":"complete-mandatory", "reserve_tokens":8192},
                    "rationale":"Unaccepted offline diagnostic"})
                    .to_string();
                // Historical proposals must stay readable even when a current
                // producer no longer supports their options.
                let value = validate_proposal(profile, &specification, &bytes).unwrap();
                assert_eq!(
                    validate_current_compiler_options(profile, &value).is_ok(),
                    profile != "sempai" || view == "literal-reference/v5-contextual-sharing",
                    "{profile}/{view}"
                );
            }
        }
    }
}
