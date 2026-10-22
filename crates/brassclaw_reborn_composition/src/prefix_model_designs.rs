//! Retain model-contract and declarative design proposals using existing provider
//! identities. Unknown artifacts/cache facts stay Unknown. No provider dispatch,
//! design acceptance, generation selection or activation occurs in this store.
use crate::pg_provider_repo::PgProviderRepo;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    CapturePrefixModelRequest, InterceptorConfigServiceError as Error, PrefixDesignProposal,
    PrefixModelDesignCatalogue, PrefixModelSpecification, PrefixModelTarget,
    RetainPrefixDesignProposalRequest,
};
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn bad() -> Error {
    Error::InvalidRequest {
        reason: "invalid model/design reference or unsupported design options".into(),
    }
}
fn hash(value: &str) -> Result<(), Error> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad());
    }
    Ok(())
}
pub(crate) struct PgPrefixModelDesigns {
    pool: Arc<PgPool>,
    namespace: String,
}
impl PgPrefixModelDesigns {
    pub(crate) fn new(pool: Arc<PgPool>, namespace: String) -> Self {
        Self { pool, namespace }
    }
    pub(crate) async fn catalogue(
        &self,
        targets: Vec<PrefixModelTarget>,
    ) -> Result<PrefixModelDesignCatalogue, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let specifications=client.query("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE provider_namespace=$1 ORDER BY created_at,specification_id",&[&self.namespace]).await.map_err(|_|Error::Unavailable)?.iter().map(specification).collect::<Result<Vec<_>,_>>()?;
        let proposals=client.query("SELECT p.proposal_id,p.profile_name,p.specification_id,p.proposal_bytes FROM reborn_prefix_design_proposals p JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE s.provider_namespace=$1 ORDER BY p.created_at,p.proposal_id",&[&self.namespace]).await.map_err(|_|Error::Unavailable)?.iter().map(proposal).collect::<Result<Vec<_>,_>>()?;
        Ok(PrefixModelDesignCatalogue {
            targets,
            specifications,
            proposals,
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
        drop(client);
        if previous.len() > 1 {
            return Err(Error::Conflict);
        }
        if let Some(row) = previous.first() {
            // A lost-response retry returns the exact historical contract, even
            // after selection changes. It neither captures nor qualifies the new target.
            return specification(row);
        }
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
        let revision = i64::try_from(retained.revision).map_err(|_| bad())?;
        let context = if request.model == retained.definition.default_model {
            retained.definition.context_window_tokens
        } else {
            None
        };
        let bytes=json!({"format":"brassclaw-prefix-model-specification/1","provider_namespace":self.namespace,"provider_id":request.provider_id,"provider_revision":revision,"model":request.model,"protocol":retained.definition.protocol,
            "configured_context_window":context,"context_evidence":"provider-configuration-only",
            "model_token_limits":retained.definition.model_token_limits.0.get(&request.model),
            "model_token_limits_owner":"provider-definition-revision",
            "weights_identity":null,"config_identity":null,"tokenizer_identity":null,"template_identity":null,
            "architecture":null,"attention_recurrent_state":null,"runtime_cache":null,
            "qualification":"configured-model-contract-only"}).to_string();
        let id = sha(bytes.as_bytes());
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        client.execute("INSERT INTO reborn_prefix_model_specifications(specification_id,provider_namespace,provider_id,provider_revision,model,specification_bytes) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",&[&id,&self.namespace,&request.provider_id,&revision,&request.model,&bytes]).await.map_err(|_|Error::Unavailable)?;
        let row=client.query_one("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE specification_id=$1",&[&id]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, String>(4) != bytes {
            return Err(Error::Conflict);
        }
        specification(&row)
    }
    pub(crate) async fn get_specification(
        &self,
        id: &str,
    ) -> Result<PrefixModelSpecification, Error> {
        hash(id)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row = client.query_opt("SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE specification_id=$1 AND provider_namespace=$2", &[&id, &self.namespace]).await.map_err(|_| Error::Unavailable)?.ok_or(Error::Conflict)?;
        specification(&row)
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
    let value = strict_json(
        bytes,
        ContractLimits {
            max_depth: 32,
            max_nodes: 32768,
            max_bytes: 1048576,
        },
    )
    .map_err(|_| bad())?;
    let fields = ["format", "profile", "specification", "design", "rationale"];
    let object = value.as_object().ok_or_else(bad)?;
    if object.len() != fields.len()
        || fields.iter().any(|key| !object.contains_key(*key))
        || value["format"] != "brassclaw-prefix-design-proposal/1"
        || value["profile"] != profile
        || value["specification"] != specification
        || value["rationale"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
    {
        return Err(bad());
    }
    let design = value["design"].as_object().ok_or_else(bad)?;
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
    if profile == "sempai"
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
mod tests {
    use super::*;

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
