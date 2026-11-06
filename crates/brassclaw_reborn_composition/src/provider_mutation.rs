//! Original operator publication identity, inside the existing provider commit.
//! No event bus, alternative catalogue, execution authority or client-uptake claim.

/// Exact optional links recorded in the ONE provider mutation INSERT.
/// Missing credential_head_ref means no native login binding; a hex-64 value
/// ties this login publication to its immutable native credential head.
#[derive(Default)]
pub(crate) struct ProviderMutationLinks {
    pub(crate) bootstrap_job_id: Option<uuid::Uuid>,
    /// SHA-256 hex-64 of the native credential head. Only set by Root's native
    /// Codex publication path after the token is authenticated. Callers that do
    /// not own native publication always pass None; the DB CHECK enforces the
    /// codex-device-login/1 kind constraint independently.
    pub(crate) credential_head_ref: Option<String>,
}

use brassclaw_product_workflow::{
    LlmConfigServiceError, ProviderMutationReceipt, UpsertLlmProviderRequest,
};
use brassclaw_secrets::SecretMaterial;
use secrecy::ExposeSecret as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) struct ProviderMutation {
    id: uuid::Uuid,
    operation: &'static str,
    request: Value,
    proof: SecretMaterial,
}

impl ProviderMutation {
    pub(crate) fn upsert(
        request: &UpsertLlmProviderRequest,
    ) -> Result<Self, LlmConfigServiceError> {
        let key = request
            .api_key
            .as_ref()
            .filter(|key| !super::llm_config_service::is_masked_sentinel(key));
        let mut payload = json!({
            "id":request.id, "expected_revision":request.expected_revision,
            "name":request.name, "adapter":request.adapter, "base_url":request.base_url,
            "default_model":request.default_model, "set_active":request.set_active,
            "model":request.model, "token_budget":request.token_budget,
            "response_cache":request.response_cache, "model_discovery":request.model_discovery,
            "request_policy":request.request_policy, "embedding_models":request.embedding_models,
            "model_token_limits":request.model_token_limits, "has_new_key":key.is_some(),
        });
        // Omission preserves the original proof bytes of pre-CAS requests.
        if let Some(reference) = &request.expected_selection_ref {
            payload["expected_selection_ref"] = json!(reference);
        }
        Self::new(request.mutation_id, "upsert", payload, key)
    }

    pub(crate) fn new(
        id: uuid::Uuid,
        operation: &'static str,
        mut request: Value,
        key: Option<&SecretMaterial>,
    ) -> Result<Self, LlmConfigServiceError> {
        if id.is_nil() {
            return Err(LlmConfigServiceError::InvalidRequest {
                field: Some("mutation_id".into()),
                reason: "Use a nonempty original mutation ID.".into(),
            });
        }
        // Keep proof bytes stable across serde_json preserve_order feature
        // unification and PostgreSQL JSONB's object-key ordering.
        request.sort_all_objects();
        let bytes = serde_json::to_vec(&request).map_err(|_| LlmConfigServiceError::Unavailable)?;
        let mut digest = Sha256::new();
        digest.update(b"brassclaw-provider-mutation/1\0");
        digest.update(id.as_bytes());
        digest.update((operation.len() as u64).to_be_bytes());
        digest.update(operation.as_bytes());
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(&bytes);
        digest.update([u8::from(key.is_some())]);
        if let Some(key) = key {
            digest.update(key.expose_secret().as_bytes());
        }
        // This digest contains credential identity: encrypt it as secret
        // material, never log it or put it in a public JSON/SQL record.
        let proof = SecretMaterial::from(format!("{:x}", digest.finalize()));
        Ok(Self {
            id,
            operation,
            request,
            proof,
        })
    }

    /// Serialize equal identities across processes before checking the original.
    /// A hash collision only causes extra serialization, never an identity match.
    pub(crate) async fn lock(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        tenant: &str,
    ) -> Result<(), LlmConfigServiceError> {
        let mut digest = Sha256::new();
        digest.update(b"provider-mutation-lock/1\0");
        digest.update(tenant.as_bytes());
        digest.update(self.id.as_bytes());
        let bytes = digest.finalize();
        let mut lock = [0u8; 8];
        lock.copy_from_slice(&bytes[..8]);
        let lock = i64::from_be_bytes(lock);
        transaction
            .query_one("SELECT pg_advisory_xact_lock($1)", &[&lock])
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(())
    }

    pub(crate) async fn original(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        tenant: &str,
        keys: &crate::LlmKeyStore,
    ) -> Result<Option<ProviderMutationReceipt>, LlmConfigServiceError> {
        let Some(row) = transaction.query_opt(
            "SELECT mutation_id,operation,request,provider_id,provider_revision,role_selections,actor_id,bootstrap_job_id,credential_head_ref \
             FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1 AND mutation_id=$2",
            &[&tenant, &self.id],
        ).await.map_err(|_| LlmConfigServiceError::Unavailable)? else { return Ok(None) };
        let operation: String = row
            .try_get("operation")
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let request: Value = row
            .try_get("request")
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if operation != self.operation
            || request != self.request
            || !keys
                .matches_mutation_proof(transaction, self.id, &self.proof)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
        {
            return Err(LlmConfigServiceError::Conflict { reason:"Mutation ID belongs to a different original request; inspect that original before submitting a new change.".into() });
        }
        decode(transaction, tenant, row).await.map(Some)
    }

    pub(crate) async fn record(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        tenant: &str,
        actor: &str,
        provider: Option<(&str, u64)>,
        keys: &crate::LlmKeyStore,
    ) -> Result<ProviderMutationReceipt, LlmConfigServiceError> {
        self.record_with_links(
            transaction,
            tenant,
            actor,
            provider,
            keys,
            ProviderMutationLinks::default(),
        )
        .await
    }

    #[cfg(feature = "skills-db")]
    pub(crate) fn id(&self) -> uuid::Uuid {
        self.id
    }
    pub(crate) async fn record_with_links(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        tenant: &str,
        actor: &str,
        provider: Option<(&str, u64)>,
        keys: &crate::LlmKeyStore,
        links: ProviderMutationLinks,
    ) -> Result<ProviderMutationReceipt, LlmConfigServiceError> {
        let (provider_id, revision) = match provider {
            Some((id, revision)) => (
                Some(id),
                Some(i64::try_from(revision).map_err(|_| LlmConfigServiceError::Unavailable)?),
            ),
            None => (None, None),
        };
        // credential_head_ref must be hex-64 when supplied; the DB CHECK enforces
        // the kind/provider/revision constraints independently.
        if let Some(head) = &links.credential_head_ref
            && (head.len() != 64 || !head.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        let roles = crate::db_config::provider_role_selections(transaction, tenant)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let roles = serde_json::to_value(roles).map_err(|_| LlmConfigServiceError::Unavailable)?;
        keys.record_mutation_proof(transaction, self.id, self.proof.clone())
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let row = transaction.query_one(
            "INSERT INTO brassclaw_llm_provider_mutations \
             (tenant_id,mutation_id,operation,request,actor_id,provider_id,provider_revision,role_selections,bootstrap_job_id,credential_head_ref) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING mutation_id,operation,request,provider_id,provider_revision,role_selections,actor_id,bootstrap_job_id,credential_head_ref",
            &[&tenant,&self.id,&self.operation,&self.request,&actor,&provider_id,&revision,&roles,&links.bootstrap_job_id,&links.credential_head_ref],
        ).await.map_err(|_| LlmConfigServiceError::Unavailable)?;
        decode(transaction, tenant, row).await
    }
}

pub(crate) async fn read_original(
    client: &(impl tokio_postgres::GenericClient + Sync),
    tenant: &str,
    id: uuid::Uuid,
) -> Result<ProviderMutationReceipt, LlmConfigServiceError> {
    let row = client.query_opt("SELECT mutation_id,operation,request,provider_id,provider_revision,role_selections,actor_id,bootstrap_job_id,credential_head_ref FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1 AND mutation_id=$2", &[&tenant,&id])
        .await.map_err(|_| LlmConfigServiceError::Unavailable)?.ok_or(LlmConfigServiceError::NotFound)?;
    decode(client, tenant, row).await
}

async fn decode<C: tokio_postgres::GenericClient + Sync>(
    client: &C,
    tenant: &str,
    row: tokio_postgres::Row,
) -> Result<ProviderMutationReceipt, LlmConfigServiceError> {
    let error = |_| LlmConfigServiceError::Unavailable;
    let revision: Option<i64> = row.try_get("provider_revision").map_err(error)?;
    let roles: std::collections::BTreeMap<String, String> =
        serde_json::from_value(row.try_get::<_, Value>("role_selections").map_err(error)?)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
    let operation: String = row.try_get("operation").map_err(error)?;
    let request: Value = row.try_get("request").map_err(error)?;
    let selected_role = if operation == "select" {
        Some(
            serde_json::from_value(
                request
                    .get("role")
                    .cloned()
                    .ok_or(LlmConfigServiceError::Unavailable)?,
            )
            .map_err(|_| LlmConfigServiceError::Unavailable)?,
        )
    } else {
        None
    };
    let bootstrap_job_id: Option<uuid::Uuid> = row.try_get("bootstrap_job_id").map_err(error)?;
    #[cfg(feature = "skills-db")]
    let bootstrap_target = if let Some(job_id) = bootstrap_job_id {
        // Detect native Codex login by its kind and the presence of a
        // published credential head reference in the same immutable row.
        let credential_head_ref: Option<String> =
            row.try_get("credential_head_ref").map_err(error)?;
        let is_native_login = operation == "upsert"
            && request["kind"] == serde_json::json!(crate::provider_codex_credentials::LOGIN_KIND)
            && credential_head_ref.is_some();
        let job = crate::prefix_job_definitions::Store::get_on(client, job_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::Unavailable)?;
        let job_scope = job
            .origin
            .chat_scope()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let actor: String = row.try_get("actor_id").map_err(error)?;
        let mutation_id: uuid::Uuid = row.try_get("mutation_id").map_err(error)?;
        let provider_id: Option<String> = row.try_get("provider_id").map_err(error)?;
        let original = brassclaw_skills::value_contract::strict_json(
            job.input["capture_bytes"]
                .as_str()
                .ok_or(LlmConfigServiceError::Unavailable)?,
            job.original_input_limits,
        )
        .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let target = &original["packet"]["payload"]["target"];
        // Compare with the actual saved ORIGINAL role values, not request.model
        // echoes, guessed defaults or today's role. Reuse the role owner's key
        // grammar rather than duplicating configuration names in this reader.
        let role = match selected_role {
            Some(brassclaw_llm::ProviderRole::Kohai) => "kohai",
            Some(brassclaw_llm::ProviderRole::Sempai) => "sempai",
            None if operation == "upsert" && request["set_active"] == true => "kohai",
            // Native login records its role intent inside the original context
            // field; absent means the operator selected the default Kohai role.
            None if is_native_login => match request["context"]["request"]["role"].as_str() {
                Some("kohai") | None => "kohai",
                Some("sempai") => "sempai",
                _ => return Err(LlmConfigServiceError::Unavailable),
            },
            _ => return Err(LlmConfigServiceError::Unavailable),
        };
        let role_keys = crate::db_config::model_role_keys(role)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;

        let crate::prefix_job_definitions::Target::ModelSetup {
            specification,
            setup_revision,
        } = &job.target
        else {
            return Err(LlmConfigServiceError::Unavailable);
        };
        if job.id != job_id
            || job.operation != "bootstrap_setup"
            || job.mode != "target"
            || job.created_by != actor
            || job_scope.tenant_id.as_str() != tenant
            || job.request["origin_mutation"] != mutation_id.to_string()
            || original["origin_mutation"] != mutation_id.to_string()
            || original["job_id"] != job_id.to_string()
            || target["specification"] != *specification
            || target["provider_id"].as_str() != provider_id.as_deref()
            || target["provider_revision"].as_u64() != revision.and_then(|v| u64::try_from(v).ok())
            || target["provider_id"].as_str() != roles.get(role_keys[0]).map(String::as_str)
            || target["model"].as_str() != roles.get(role_keys[1]).map(String::as_str)
            || roles.contains_key(role_keys[2])
            || roles.contains_key(role_keys[3])
            || original["packet"]["payload"]["executing_pair"] != *target
            || original["setup"]["revision"].as_u64() != Some(*setup_revision)
            || original["setup"]["specification"] != *specification
            || job.request["executing_specification"] != *specification
            || !((operation == "select"
                && matches!(request["role"].as_str(), Some("kohai" | "sempai")))
                || (operation == "upsert" && request["set_active"] == true)
                || is_native_login)
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        Some(brassclaw_product_workflow::PrefixJobTarget::ModelSetup {
            specification: specification.clone(),
            setup_revision: *setup_revision,
        })
    } else {
        None
    };
    #[cfg(not(feature = "skills-db"))]
    let bootstrap_target = {
        if bootstrap_job_id.is_some() {
            return Err(LlmConfigServiceError::Unavailable);
        }
        let _ = (client, tenant);
        None
    };
    Ok(ProviderMutationReceipt {
        bootstrap_job_id,
        bootstrap_target,
        mutation_id: row.try_get("mutation_id").map_err(error)?,
        operation,
        selected_role,
        provider_id: row.try_get("provider_id").map_err(error)?,
        provider_revision: revision
            .map(u64::try_from)
            .transpose()
            .map_err(|_| LlmConfigServiceError::Unavailable)?,
        role_selections: roles,
    })
}

#[cfg(test)]
#[path = "provider_mutation_tests.rs"]
mod tests;
