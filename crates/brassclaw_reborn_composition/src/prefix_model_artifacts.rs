//! Exact local model artifacts, independent of prefix packages. Retention proves
//! bytes/integrity only; a provider/model-name association is not runtime identity.
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    InterceptorConfigServiceError as Error, PrefixModelArtifactFile, PrefixModelArtifactSet,
    RetainPrefixModelArtifactsRequest, WebUiAuthenticatedCaller,
};
use brassclaw_skills::value_contract::{EXECUTION_DATA_LIMITS, strict_json};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

const FILES: [&str; 8] = [
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "added_tokens.json",
    "vocab.json",
    "merges.txt",
    "chat_template.jinja",
];
fn sha(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
fn bad() -> Error {
    Error::InvalidRequest {
        reason: "invalid or unsupported model artifact set".into(),
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

/// Import provenance is data, never a component/usage approval. Only trusted
/// installation code can choose the fixture variant; public imports always
/// construct Operator. The qualification owner must still verify actual fixture
/// bytes, dispatch behavior and its independent immutable approval evidence.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ArtifactImportOrigin {
    Operator {
        actor: String,
    },
    SystemSeedFixture {
        actor: String,
        producer: String,
        source_checksum: String,
    },
}
impl ArtifactImportOrigin {
    fn validate(&self, namespace: &str) -> Result<(), Error> {
        let actor = match self {
            Self::Operator { actor } => actor,
            Self::SystemSeedFixture {
                actor,
                producer,
                source_checksum,
            } => {
                if namespace != crate::bootstrap_validation::NAMESPACE
                    || producer != "brassclaw-model-encoding-bootstrap/1"
                {
                    return Err(bad());
                }
                hash(source_checksum)?;
                actor
            }
        };
        if actor.is_empty() {
            return Err(bad());
        }
        Ok(())
    }
}

fn validate_binding(
    value: &Value,
    specification: &str,
    id: &str,
    namespace: &str,
) -> Result<(), Error> {
    if value["format"] != "brassclaw-prefix-model-artifact-binding/1"
        || value.as_object().is_none_or(|v| v.len() != 5)
        || value["specification"] != specification
        || value["artifact_set"] != id
        || value["qualification"] != "unverified-model-artifacts"
    {
        return Err(Error::Unavailable);
    }
    let origin: ArtifactImportOrigin =
        serde_json::from_value(value["origin"].clone()).map_err(|_| Error::Unavailable)?;
    origin.validate(namespace).map_err(|_| Error::Unavailable)
}
fn package(bytes: &str) -> Result<(String, Vec<PrefixModelArtifactFile>), Error> {
    let value = strict_json(bytes, EXECUTION_DATA_LIMITS).map_err(|_| bad())?;
    if value.as_object().is_none_or(|v| v.len() != 2)
        || !matches!(
            value["format"].as_str(),
            Some(
                "brassclaw-prefix-model-artifact-set/1"
                    | "brassclaw-prefix-model-artifact-import/1"
            )
        )
    {
        return Err(bad());
    }
    let artifacts = value["artifacts"].as_object().ok_or_else(bad)?;
    if !["config.json", "tokenizer.json", "tokenizer_config.json"]
        .iter()
        .all(|key| artifacts.contains_key(*key))
        || artifacts.keys().any(|key| !FILES.contains(&key.as_str()))
    {
        return Err(bad());
    }
    let mut files = BTreeMap::new();
    let mut inventory = Vec::new();
    let mut total = 0usize;
    for (name, artifact) in artifacts {
        if artifact.as_object().is_none_or(|v| {
            v.len()
                != if value["format"] == "brassclaw-prefix-model-artifact-import/1" {
                    2
                } else {
                    3
                }
        }) || artifact["encoding"] != "base64"
        {
            return Err(bad());
        }
        let data = STANDARD
            .decode(artifact["data"].as_str().ok_or_else(bad)?)
            .map_err(|_| bad())?;
        let digest = sha(&data);
        if data.is_empty()
            || (value["format"] == "brassclaw-prefix-model-artifact-set/1"
                && artifact["sha256"] != digest)
            || std::str::from_utf8(&data).is_err()
        {
            return Err(bad());
        }
        total = total.checked_add(data.len()).ok_or_else(bad)?;
        if total > EXECUTION_DATA_LIMITS.max_bytes {
            return Err(bad());
        }
        files.insert(
            name.clone(),
            json!({"encoding":"base64","sha256":digest,"data":STANDARD.encode(&data)}),
        );
        inventory.push(PrefixModelArtifactFile {
            name: name.clone(),
            sha256: digest,
            bytes: data.len() as u64,
        });
    }
    let encoded = crate::prefix_generations::canonical(
        &json!({"format":"brassclaw-prefix-model-artifact-set/1","artifacts":files}),
    )
    .map_err(|_| bad())?;
    if encoded.len() > EXECUTION_DATA_LIMITS.max_bytes {
        return Err(bad());
    }
    Ok((encoded, inventory))
}
fn summary(
    specification: &str,
    id: String,
    files: Vec<PrefixModelArtifactFile>,
    binding_bytes: String,
) -> PrefixModelArtifactSet {
    PrefixModelArtifactSet {
        id,
        specification: specification.into(),
        files,
        binding_bytes,
        qualification: "unverified-model-artifacts".into(),
    }
}
pub(crate) struct PgPrefixModelArtifacts {
    pool: Arc<PgPool>,
    namespace: String,
}
impl PgPrefixModelArtifacts {
    pub(crate) fn new(pool: Arc<PgPool>, namespace: String) -> Self {
        Self { pool, namespace }
    }
    pub(crate) async fn retain(
        &self,
        request: RetainPrefixModelArtifactsRequest,
        caller: &WebUiAuthenticatedCaller,
    ) -> Result<PrefixModelArtifactSet, Error> {
        self.retain_with_origin(
            request,
            ArtifactImportOrigin::Operator {
                actor: caller.user_id.as_str().to_owned(),
            },
        )
        .await
    }

    /// Private unqualified-data import shared by operator and installation
    /// owners. No host/API accepts caller-selected provenance or approval here.
    pub(super) async fn retain_with_origin(
        &self,
        request: RetainPrefixModelArtifactsRequest,
        origin: ArtifactImportOrigin,
    ) -> Result<PrefixModelArtifactSet, Error> {
        origin.validate(&self.namespace)?;
        hash(&request.specification)?;
        let (bytes, files) = package(&request.artifact_set_bytes)?;
        let id = sha(bytes.as_bytes());
        let binding=json!({"format":"brassclaw-prefix-model-artifact-binding/1","specification":request.specification,"artifact_set":id,
            "origin":origin,"qualification":"unverified-model-artifacts"}).to_string();
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        if tx.query_opt("SELECT specification_id FROM reborn_prefix_model_specifications WHERE specification_id=$1 AND provider_namespace=$2",&[&request.specification,&self.namespace]).await.map_err(|_|Error::Unavailable)?.is_none() {return Err(Error::Conflict);}
        tx.execute("INSERT INTO reborn_prefix_model_artifact_sets(artifact_set_id,artifact_set_bytes,inventory_bytes,inventory_checksum) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",&[&id,&bytes,&serde_json::to_string(&files).map_err(|_|bad())?,&sha(serde_json::to_string(&files).map_err(|_|bad())?.as_bytes())]).await.map_err(|_|Error::Unavailable)?;
        let row=tx.query_one("SELECT artifact_set_bytes,inventory_bytes,inventory_checksum FROM reborn_prefix_model_artifact_sets WHERE artifact_set_id=$1",&[&id]).await.map_err(|_|Error::Unavailable)?;
        let inventory = serde_json::to_string(&files).map_err(|_| bad())?;
        if row.get::<_, String>(0) != bytes
            || row.get::<_, String>(1) != inventory
            || row.get::<_, String>(2) != sha(inventory.as_bytes())
        {
            return Err(Error::Conflict);
        }
        tx.execute("INSERT INTO reborn_prefix_model_artifact_bindings(specification_id,artifact_set_id,binding_bytes,checksum) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",&[&request.specification,&id,&binding,&sha(binding.as_bytes())]).await.map_err(|_|Error::Unavailable)?;
        let actual:String=tx.query_one("SELECT binding_bytes FROM reborn_prefix_model_artifact_bindings WHERE specification_id=$1 AND artifact_set_id=$2",&[&request.specification,&id]).await.map_err(|_|Error::Unavailable)?.get(0);
        if actual != binding {
            return Err(Error::Conflict);
        }
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        Ok(summary(&request.specification, id, files, binding))
    }
    pub(crate) async fn get(
        &self,
        specification: &str,
        id: &str,
    ) -> Result<brassclaw_product_workflow::PrefixModelArtifactDetail, Error> {
        hash(specification)?;
        hash(id)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row=client.query_opt("SELECT a.artifact_set_bytes,a.inventory_bytes,a.inventory_checksum,b.binding_bytes,b.checksum FROM reborn_prefix_model_artifact_sets a JOIN reborn_prefix_model_artifact_bindings b USING(artifact_set_id) JOIN reborn_prefix_model_specifications s USING(specification_id) WHERE b.specification_id=$1 AND a.artifact_set_id=$2 AND s.provider_namespace=$3",&[&specification,&id,&self.namespace]).await.map_err(|_|Error::Unavailable)?.ok_or(Error::Conflict)?;
        let bytes: String = row.get(0);
        let inventory: String = row.get(1);
        let inventory_checksum: String = row.get(2);
        let binding: String = row.get(3);
        let checksum: String = row.get(4);
        let (canonical, files) = package(&bytes).map_err(|_| Error::Unavailable)?;
        if canonical != bytes
            || sha(bytes.as_bytes()) != id
            || sha(inventory.as_bytes()) != inventory_checksum
            || serde_json::to_string(&files).map_err(|_| Error::Unavailable)? != inventory
            || sha(binding.as_bytes()) != checksum
        {
            return Err(Error::Unavailable);
        }
        let value: Value = strict_json(
            &binding,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .map_err(|_| Error::Unavailable)?;
        validate_binding(&value, specification, id, &self.namespace)?;
        Ok(brassclaw_product_workflow::PrefixModelArtifactDetail {
            summary: summary(specification, id.into(), files, binding),
            artifact_set_bytes: bytes,
        })
    }
    pub(crate) async fn list(&self) -> Result<Vec<PrefixModelArtifactSet>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let rows=client.query("SELECT b.specification_id,b.artifact_set_id,b.binding_bytes,b.checksum,a.inventory_bytes,a.inventory_checksum FROM reborn_prefix_model_artifact_bindings b JOIN reborn_prefix_model_artifact_sets a USING(artifact_set_id) JOIN reborn_prefix_model_specifications s USING(specification_id) WHERE s.provider_namespace=$1 ORDER BY b.created_at,b.specification_id,b.artifact_set_id",&[&self.namespace]).await.map_err(|_|Error::Unavailable)?;
        let mut output = Vec::new();
        for row in rows {
            let specification: String = row.get(0);
            let id: String = row.get(1);
            let binding: String = row.get(2);
            let checksum: String = row.get(3);
            let inventory: String = row.get(4);
            let inventory_checksum: String = row.get(5);
            if sha(inventory.as_bytes()) != inventory_checksum
                || sha(binding.as_bytes()) != checksum
            {
                return Err(Error::Unavailable);
            }
            let files: Vec<PrefixModelArtifactFile> =
                serde_json::from_str(&inventory).map_err(|_| Error::Unavailable)?;
            let value: Value = strict_json(
                &binding,
                brassclaw_skills::component_revision::REVISION_LIMITS,
            )
            .map_err(|_| Error::Unavailable)?;
            validate_binding(&value, &specification, &id, &self.namespace)?;
            output.push(summary(&specification, id, files, binding));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(origin: Value) -> Value {
        json!({"format":"brassclaw-prefix-model-artifact-binding/1",
            "specification":"spec","artifact_set":"assets",
            "qualification":"unverified-model-artifacts","origin":origin})
    }

    #[test]
    fn model_artifact_origin_preserves_operator_and_contains_seed_fixtures() {
        let operator = binding(json!({"kind":"operator","actor":"operator"}));
        for namespace in ["instance", crate::bootstrap_validation::NAMESPACE] {
            validate_binding(&operator, "spec", "assets", namespace).unwrap();
        }
        let fixture = binding(
            json!({"kind":"system_seed_fixture","actor":"installation-owner",
            "producer":"brassclaw-model-encoding-bootstrap/1","source_checksum":"a".repeat(64)}),
        );
        validate_binding(
            &fixture,
            "spec",
            "assets",
            crate::bootstrap_validation::NAMESPACE,
        )
        .unwrap();
        assert!(validate_binding(&fixture, "spec", "assets", "instance").is_err());
        for (field, value) in [
            ("actor", json!("")),
            ("producer", json!("untrusted")),
            ("source_checksum", json!("A".repeat(64))),
            ("source_checksum", Value::Null),
            ("q1_ref", json!("fabricated")),
        ] {
            let mut changed = fixture.clone();
            changed["origin"][field] = value;
            assert!(
                validate_binding(
                    &changed,
                    "spec",
                    "assets",
                    crate::bootstrap_validation::NAMESPACE
                )
                .is_err(),
                "{field}"
            );
        }
        let mut changed = operator;
        changed["origin"]["source_checksum"] = json!("a".repeat(64));
        assert!(validate_binding(&changed, "spec", "assets", "instance").is_err());
        let mut changed = fixture;
        changed["qualification"] = json!("approved");
        assert!(
            validate_binding(
                &changed,
                "spec",
                "assets",
                crate::bootstrap_validation::NAMESPACE
            )
            .is_err()
        );
    }

    #[test]
    fn model_artifact_package_cannot_supply_origin_or_approval() {
        let files: serde_json::Map<String, Value> =
            ["config.json", "tokenizer.json", "tokenizer_config.json"]
                .into_iter()
                .map(|name| {
                    (
                        name.into(),
                        json!({"encoding":"base64","data":STANDARD.encode(b"{} ")}),
                    )
                })
                .collect();
        let mut value =
            json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files});
        package(&value.to_string()).unwrap();
        value["origin"] = json!({"kind":"system_seed_fixture"});
        assert!(package(&value.to_string()).is_err());
        value.as_object_mut().unwrap().remove("origin");
        value["qualification"] = json!("approved");
        assert!(package(&value.to_string()).is_err());
    }
}
