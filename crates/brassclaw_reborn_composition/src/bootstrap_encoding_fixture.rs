//! Fixed data-only fixture for closed installation qualification. It uses the
//! existing provider-revision/model-asset stores, never a serving selection or
//! provider request. The independent oracle does not call the encoding engine.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(super) const PRODUCER: &str = "brassclaw-model-encoding-bootstrap/1";
pub(super) const TEMPLATE: &str = "{{ messages[0].content }}";
pub(super) const EFFECTIVE: &str = "hello {{ messages[0].content }}";
pub(super) fn checksum(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub(super) fn tokenizer() -> String {
    json!({"version":"1.0","truncation":{"direction":"Right","max_length":4,"strategy":"LongestFirst","stride":0},
        "padding":{"strategy":{"Fixed":8},"direction":"Right","pad_to_multiple_of":null,"pad_id":0,"pad_type_id":0,"pad_token":"[UNK]"},
        "added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"WhitespaceSplit"},"post_processor":null,"decoder":null,
        "model":{"type":"WordLevel","vocab":{"[UNK]":0,"hello":1,"world":2},"unk_token":"[UNK]"}}).to_string()
}
fn config() -> String {
    json!({"chat_template":TEMPLATE}).to_string()
}
pub(super) struct Fixture {
    pub(super) specification: String,
    pub(super) artifacts: String,
    pub(super) bad_tokenizer: String,
}
impl Fixture {
    pub(super) async fn retain(pool: Arc<PgPool>) -> Result<Self, RebornBuildError> {
        let namespace = crate::bootstrap_validation::NAMESPACE;
        let client = pool.get().await.map_err(invalid)?;
        // The migration-owned initializer, also used by PgProviderRepo, owns
        // these revisioned definitions. A data fixture needs no LLM transport.
        client
            .query_one(
                "SELECT brassclaw_initialize_provider_catalogue($1)",
                &[&namespace],
            )
            .await
            .map_err(invalid)?;
        // Reference a real retained provider revision only to satisfy its data
        // relationship. There is no provider dispatch, credential or active model.
        let row = client.query_one("SELECT provider_id,revision FROM brassclaw_llm_provider_revisions WHERE tenant_id=$1 ORDER BY provider_id,revision LIMIT 1", &[&namespace]).await.map_err(invalid)?;
        let provider: String = row.get(0);
        let revision: i64 = row.get(1);
        let model = "installation-wordlevel-data-fixture";
        let bytes = json!({"format":"brassclaw-prefix-model-specification/1","provider_namespace":namespace,
            "provider_id":provider,"provider_revision":revision,"model":model,
            "producer":PRODUCER,"qualification":"contained-data-fixture-only",
            "source_checksum":checksum(include_bytes!("bootstrap_encoding_fixture.rs")),
            "serving_identity":null}).to_string();
        let specification = checksum(bytes.as_bytes());
        client.execute("INSERT INTO reborn_prefix_model_specifications(specification_id,provider_namespace,provider_id,provider_revision,model,specification_bytes) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING", &[&specification,&namespace,&provider,&revision,&model,&bytes]).await.map_err(invalid)?;
        let actual: String = client.query_one("SELECT specification_bytes FROM reborn_prefix_model_specifications WHERE specification_id=$1", &[&specification]).await.map_err(invalid)?.get(0);
        if actual != bytes {
            return Err(invalid("encoding fixture specification differs"));
        }
        drop(client);
        let store =
            crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(pool, namespace.into());
        let mut sets = Vec::new();
        for tokenizer in [tokenizer(), "{}".into()] {
            let files: serde_json::Map<String, Value> = [
                ("config.json", "{}".to_owned()),
                ("tokenizer.json", tokenizer),
                ("tokenizer_config.json", config()),
            ]
            .into_iter()
            .map(|(name, bytes)| {
                (
                    name.into(),
                    json!({"encoding":"base64","data":STANDARD.encode(bytes.as_bytes())}),
                )
            })
            .collect();
            let retained = store.retain_with_origin(brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
                specification: specification.clone(),
                artifact_set_bytes: json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files}).to_string(),
            }, crate::prefix_model_artifacts::ArtifactImportOrigin::SystemSeedFixture {
                actor:"installation-owner".into(), producer:PRODUCER.into(), source_checksum:checksum(include_bytes!("bootstrap_encoding_fixture.rs")),
            }).await.map_err(invalid)?;
            // Re-read the same production integrity/provenance boundary before
            // it can be offered to any qualification invocation.
            store
                .get(&specification, &retained.id)
                .await
                .map_err(invalid)?;
            sets.push(retained.id);
        }
        Ok(Self {
            specification,
            artifacts: sets.remove(0),
            bad_tokenizer: sets.remove(0),
        })
    }
    pub(super) fn inputs(&self, case: &str, effective: bool) -> Value {
        let mut value = json!({"specification":self.specification,"artifact_set":if case=="bad-tokenizer" {&self.bad_tokenizer} else {&self.artifacts},
            "probes_json":json!([
                {"kind":"text","text":"hello ".repeat(2050)},
                {"kind":"chat","context_json":json!({"messages":[{"role":"user","content":"hello world"}]}).to_string()}
            ]).to_string()});
        if effective {
            value["template_utf8"] = json!(EFFECTIVE);
        }
        match case {
            "missing-assets" => value["artifact_set"] = json!("0".repeat(64)),
            "invalid-hash" => value["specification"] = json!("invalid"),
            "invalid-probes" => value["probes_json"] = json!("[]"),
            "bad-template" => {
                value["template_utf8"] = json!("{{ raise_exception('closed fixture rejection') }}");
            }
            "render-fuel" => {
                value["template_utf8"] = json!(
                    "{% for i in range(100000) %}{% for j in range(100000) %}{% set x = j %}{% endfor %}{% endfor %}"
                );
            }
            _ => {}
        }
        value
    }
    pub(super) fn verify(&self, bytes: &str, effective: bool) -> Result<Value, RebornBuildError> {
        let actual: Value = brassclaw_skills::value_contract::strict_json(
            bytes,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(invalid)?;
        if crate::prefix_generations::canonical(&actual).map_err(invalid)? != bytes {
            return Err(invalid("encoding fixture observation is not canonical"));
        }
        let ids = |values: &[u32]| {
            let mut h = Sha256::new();
            for id in values {
                h.update(id.to_le_bytes());
            }
            hex::encode(h.finalize())
        };
        let probe = |rendered: &str, values: &[u32]| json!({"rendered_sha256":checksum(rendered.as_bytes()),"rendered_bytes":rendered.len(),"token_count":values.len(),"token_ids_sha256":ids(values)});
        let expected = json!({"format":"brassclaw-model-encoding/1","tokenizer_sha256":checksum(tokenizer().as_bytes()),
            "tokenizer_config_sha256":checksum(config().as_bytes()),"chat_template_sha256":checksum(if effective { EFFECTIVE.as_bytes() } else { TEMPLATE.as_bytes() }),
            "truncation_disabled":true,"padding_disabled":true,
            "probes":[probe(&"hello ".repeat(2050),&vec![1;2050]),if effective {probe("hello hello world",&[1,1,2])} else {probe("hello world",&[1,2])}],
            "common_prefix_tokens":if effective {2} else {1},"common_prefix_sha256":if effective {ids(&[1,1])} else {ids(&[1])}});
        let valid_hash = |v: &Value| {
            v.as_str().is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        };
        if actual.as_object().is_none_or(|v| v.len() != 9)
            || actual["format"] != "brassclaw-model-encoding-observation/1"
            || actual["specification"] != self.specification
            || actual["artifact_set"] != self.artifacts
            || actual["qualification"] != "offline-measurement-only"
            || actual["template_origin"]
                != if effective {
                    "supplied-template"
                } else {
                    "model-artifact-template"
                }
            || actual["model_chat_template_sha256"] != checksum(TEMPLATE.as_bytes())
            || actual["preparation_revision"]
                .as_u64()
                .is_none_or(|r| r == 0)
            || !valid_hash(&actual["worker_checksum"])
            || actual["report"] != expected
        {
            return Err(invalid(
                "encoding fixture result differs from independent oracle",
            ));
        }
        Ok(actual)
    }
}
