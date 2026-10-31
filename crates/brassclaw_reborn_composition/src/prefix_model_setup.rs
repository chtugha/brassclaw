//! Append-only, unaccepted setup drafts in the existing model/design owner.
//! Manual edits are not provider settings, observations, approval or activation.
//! No arbitrary LLM/prefill import entry point exists here.
use super::{
    Error, PROPOSAL_LIMITS, PgPrefixModelDesigns, PrefixModelSpecification, bad, hash, sha,
};
use brassclaw_product_workflow::{
    PREFIX_SETUP_MAX_EXACT_INTEGER, PrefixModelSetupDraft, PrefixModelSetupEdit,
    PrefixModelSetupField as Field, PrefixModelSetupOrigin as Origin, PrefixModelSetupValue,
    UpdatePrefixModelSetupDraftRequest,
};
use brassclaw_skills::value_contract::strict_json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredDraft {
    format: String,
    base_revision: u64,
    edits: Vec<PrefixModelSetupEdit>,
    draft: PrefixModelSetupDraft,
}
fn unavailable() -> Error {
    Error::Unavailable
}
fn initial(specification: &PrefixModelSpecification) -> Result<PrefixModelSetupDraft, Error> {
    let value = strict_json(&specification.specification_bytes, PROPOSAL_LIMITS)
        .map_err(|_| unavailable())?;
    let mut fields = Field::ALL
        .into_iter()
        .map(|field| {
            (
                field,
                PrefixModelSetupValue {
                    value: None,
                    origin: Origin::Unset,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    // Only original configuration facts already verified by the model owner.
    // Older snapshots without this field retain Unknown, never today's values.
    if let Some(limits) = value
        .get("model_token_limits")
        .filter(|limits| !limits.is_null())
    {
        for (field, name) in [
            (Field::MaxPrefixTokens, "max_prefix_tokens"),
            (Field::MaxPromptTokens, "max_prompt_tokens"),
        ] {
            let count = limits
                .get(name)
                .and_then(Value::as_u64)
                .ok_or_else(unavailable)?;
            if !field.valid_value(Some(count)) {
                return Err(unavailable());
            }
            fields.insert(
                field,
                PrefixModelSetupValue {
                    value: Some(count),
                    origin: Origin::ProviderConfiguration,
                },
            );
        }
    }
    Ok(PrefixModelSetupDraft {
        specification: specification.id.clone(),
        revision: 0,
        operation_id: None,
        fields,
    })
}
fn normalize(mut edits: Vec<PrefixModelSetupEdit>) -> Vec<PrefixModelSetupEdit> {
    edits.sort_by_key(|edit| edit.field);
    edits
}
fn decode(row: &tokio_postgres::Row, basis: &PrefixModelSetupDraft) -> Result<StoredDraft, Error> {
    let revision = u64::try_from(row.get::<_, i64>(0)).map_err(|_| unavailable())?;
    let base_revision = u64::try_from(row.get::<_, i64>(1)).map_err(|_| unavailable())?;
    let bytes: String = row.get(2);
    if sha(bytes.as_bytes()) != row.get::<_, String>(3) {
        return Err(unavailable());
    }
    let value = strict_json(&bytes, PROPOSAL_LIMITS).map_err(|_| unavailable())?;
    let stored: StoredDraft = serde_json::from_value(value).map_err(|_| unavailable())?;
    let operation_id: uuid::Uuid = row.get(4);
    let request = UpdatePrefixModelSetupDraftRequest {
        operation_id,
        expected_revision: base_revision,
        edits: stored.edits.clone(),
    };
    if stored.format != "brassclaw-prefix-model-setup-draft/1"
        || stored.base_revision != base_revision
        || stored.draft.specification != basis.specification
        || stored.draft.revision != revision
        || stored.draft.operation_id != Some(operation_id)
        || revision == 0
        || revision > PREFIX_SETUP_MAX_EXACT_INTEGER
        || base_revision.checked_add(1) != Some(revision)
        || !request.valid()
        || stored.edits != normalize(stored.edits.clone())
        || stored.draft.fields.len() != Field::ALL.len()
        || Field::ALL
            .iter()
            .any(|field| !stored.draft.fields.contains_key(field))
    {
        return Err(unavailable());
    }
    for (field, value) in &stored.draft.fields {
        if !field.valid_value(value.value) {
            return Err(unavailable());
        }
        match value.origin {
            Origin::Unset if value.value.is_none() => {}
            Origin::ProviderConfiguration if basis.fields.get(field) == Some(value) => {}
            Origin::Manual => {}
            _ => return Err(unavailable()),
        }
    }
    for edit in &stored.edits {
        if stored.draft.fields.get(&edit.field)
            != Some(&PrefixModelSetupValue {
                value: edit.value,
                origin: Origin::Manual,
            })
        {
            return Err(unavailable());
        }
    }
    Ok(stored)
}
impl PgPrefixModelDesigns {
    async fn setup_specification<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
        specification: &str,
        locked: bool,
    ) -> Result<PrefixModelSpecification, Error> {
        let query = "SELECT specification_id,provider_id,provider_revision,model,specification_bytes FROM reborn_prefix_model_specifications WHERE specification_id=$1 AND provider_namespace=$2";
        let query = if locked {
            format!("{query} FOR UPDATE")
        } else {
            query.into()
        };
        let row = client
            .query_opt(&query, &[&specification, &self.namespace])
            .await
            .map_err(|_| unavailable())?
            .ok_or(Error::Conflict)?;
        self.verified_specifications(client, std::slice::from_ref(&row))
            .await?
            .pop()
            .ok_or_else(unavailable)
    }
    pub(crate) async fn specification_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        id: &str,
    ) -> Result<PrefixModelSpecification, Error> {
        hash(id)?;
        self.setup_specification(tx, id, false).await
    }
    /// None reads the current draft; Some pins the exact original revision.
    /// Explicit zero is the original untouched view. Missing positive revisions
    /// fail instead of falling back to a different historical/current view.
    pub(crate) async fn setup_draft(
        &self,
        specification: &str,
        revision: Option<u64>,
    ) -> Result<PrefixModelSetupDraft, Error> {
        let client = self.pool.get().await.map_err(|_| unavailable())?;
        self.read_setup_draft(&**client, specification, revision, false)
            .await
    }

    pub(crate) async fn setup_draft_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        specification: &str,
        revision: Option<u64>,
    ) -> Result<PrefixModelSetupDraft, Error> {
        self.read_setup_draft(tx, specification, revision, true)
            .await
    }

    async fn read_setup_draft<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
        specification: &str,
        revision: Option<u64>,
        locked: bool,
    ) -> Result<PrefixModelSetupDraft, Error> {
        hash(specification)?;
        if revision.is_some_and(|revision| revision > PREFIX_SETUP_MAX_EXACT_INTEGER) {
            return Err(bad());
        }
        let specification = self
            .setup_specification(client, specification, locked)
            .await?;
        let basis = initial(&specification)?;
        let row = if let Some(revision) = revision {
            if revision == 0 {
                return Ok(basis);
            }
            let revision = i64::try_from(revision).map_err(|_| bad())?;
            Some(client.query_opt("SELECT revision,base_revision,draft_bytes,draft_checksum,operation_id FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1 AND revision=$2", &[&specification.id,&revision]).await.map_err(|_| unavailable())?.ok_or(Error::Conflict)?)
        } else {
            client.query_opt("SELECT revision,base_revision,draft_bytes,draft_checksum,operation_id FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1 ORDER BY revision DESC LIMIT 1", &[&specification.id]).await.map_err(|_| unavailable())?
        };
        row.as_ref()
            .map(|row| decode(row, &basis).map(|stored| stored.draft))
            .unwrap_or(Ok(basis))
    }
    pub(crate) async fn retained_setup_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        specification: &str,
        revision: u64,
    ) -> Result<PrefixModelSetupDraft, Error> {
        self.read_setup_draft(tx, specification, Some(revision), false)
            .await
    }
    pub(crate) async fn update_setup_draft(
        &self,
        specification: &str,
        request: UpdatePrefixModelSetupDraftRequest,
    ) -> Result<PrefixModelSetupDraft, Error> {
        hash(specification)?;
        if !request.valid() {
            return Err(bad());
        }
        let expected = i64::try_from(request.expected_revision).map_err(|_| bad())?;
        let next = expected.checked_add(1).ok_or_else(bad)?;
        let edits = normalize(request.edits);
        let mut client = self.pool.get().await.map_err(|_| unavailable())?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(|_| unavailable())?;
        // Reuse the original immutable specification row as this owner's lock.
        // Every edit serializes here; no second provider head/lock service exists.
        let original = self.setup_specification(&*tx, specification, true).await?;
        let basis = initial(&original)?;
        // The immutable row, keyed by the caller's original operation, is the
        // exact data-write receipt. Matching fields from another stale action
        // are not that original operation. A retry
        // after a lost acknowledgement returns that original revision even if
        // later edits exist. Never replay it into today's draft or read latest.
        if let Some(row) = tx.query_opt("SELECT revision,base_revision,draft_bytes,draft_checksum,operation_id FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1 AND operation_id=$2", &[&specification,&request.operation_id]).await.map_err(|_| unavailable())? {
            let stored = decode(&row, &basis)?;
            if stored.base_revision != request.expected_revision || stored.edits != edits { return Err(Error::Conflict); }
            tx.commit().await.map_err(|_| unavailable())?;
            return Ok(stored.draft);
        }
        let row = tx.query_opt("SELECT revision,base_revision,draft_bytes,draft_checksum,operation_id FROM reborn_prefix_model_setup_drafts WHERE specification_id=$1 ORDER BY revision DESC LIMIT 1", &[&specification]).await.map_err(|_| unavailable())?;
        let mut current = row
            .as_ref()
            .map(|row| decode(row, &basis).map(|stored| stored.draft))
            .unwrap_or(Ok(basis.clone()))?;
        if current.revision != request.expected_revision {
            return Err(Error::Conflict);
        }
        current.revision = u64::try_from(next).map_err(|_| bad())?;
        current.operation_id = Some(request.operation_id);
        for edit in &edits {
            current.fields.insert(
                edit.field,
                PrefixModelSetupValue {
                    value: edit.value,
                    origin: Origin::Manual,
                },
            );
        }
        let stored = StoredDraft {
            format: "brassclaw-prefix-model-setup-draft/1".into(),
            base_revision: request.expected_revision,
            edits,
            draft: current,
        };
        let value = serde_json::to_value(stored).map_err(|_| unavailable())?;
        let bytes = crate::prefix_generations::canonical(&value).map_err(|_| unavailable())?;
        if bytes.len() > PROPOSAL_LIMITS.max_bytes {
            return Err(bad());
        }
        let checksum = sha(bytes.as_bytes());
        let row = tx.query_one("INSERT INTO reborn_prefix_model_setup_drafts(specification_id,revision,base_revision,draft_bytes,draft_checksum,operation_id) VALUES($1,$2,$3,$4,$5,$6) RETURNING revision,base_revision,draft_bytes,draft_checksum,operation_id", &[&specification,&next,&expected,&bytes,&checksum,&request.operation_id]).await.map_err(|_| unavailable())?;
        if row.get::<_, String>(2) != bytes || row.get::<_, String>(3) != checksum {
            return Err(unavailable());
        }
        let actual = decode(&row, &basis)?.draft;
        tx.commit().await.map_err(|_| unavailable())?;
        Ok(actual)
    }
}

#[cfg(test)]
#[path = "prefix_model_setup_tests.rs"]
mod tests;
