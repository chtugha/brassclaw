//! Independent small syntax/data oracle for the complete packaged measurement
//! workflow. This is not an Ornith, serving-model or cache qualification.
use crate::{
    RebornBuildError,
    bootstrap_encoding_fixture::{checksum, tokenizer},
    bootstrap_reply_validation::invalid,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(super) const CASES: &[&str] = &[
    "fits",
    "does-not-fit",
    "duplicate",
    "wrong-original",
    "unsupported-template",
    "denied",
    "bad-assets",
    "bad-input",
];
pub(super) const REFERENCE: &str = "hello hello";
pub(super) const TEMPLATE: &str = r#"{%- if tools -%}
{{- '<|im_start|>system\n' }}
{{- tools|tojson }}
{{- '\n<|im_end|>\n' }}
{%- endif -%}
{%- for message in messages -%}
{{- '<|im_start|>' ~ message['role'] ~ '\n' ~ message['content'] ~ '\n<|im_end|>\n' }}
{%- endfor -%}
{%- if add_generation_prompt -%}
{{- '<|im_start|>assistant\n' }}
{%- endif -%}"#;
fn config() -> String {
    json!({"chat_template":TEMPLATE}).to_string()
}
pub(super) struct Fixture {
    pub(super) specification: String,
    pub(super) artifacts: String,
}
impl Fixture {
    pub(super) async fn retain(pool: Arc<PgPool>) -> Result<Arc<Self>, RebornBuildError> {
        let base = crate::bootstrap_encoding_fixture::Fixture::retain(pool.clone()).await?;
        let files: serde_json::Map<String, Value> = [
            ("config.json", "{}".to_owned()),
            ("tokenizer.json", tokenizer()),
            ("tokenizer_config.json", config()),
        ]
        .into_iter()
        .map(|(name, text)| {
            (
                name.into(),
                json!({"encoding":"base64","data":STANDARD.encode(text.as_bytes())}),
            )
        })
        .collect();
        let store = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
            pool,
            crate::bootstrap_validation::NAMESPACE.into(),
        );
        let assets=store.retain_with_origin(brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
            specification:base.specification.clone(),artifact_set_bytes:json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files}).to_string(),
        },crate::prefix_model_artifacts::ArtifactImportOrigin::SystemSeedFixture {actor:"installation-owner:prefix-measurement".into(),producer:crate::bootstrap_encoding_fixture::PRODUCER.into(),source_checksum:checksum(include_bytes!("bootstrap_measurement_fixture.rs"))}).await.map_err(invalid)?;
        store
            .get(&base.specification, &assets.id)
            .await
            .map_err(invalid)?;
        Ok(Arc::new(Self {
            specification: base.specification,
            artifacts: assets.id,
        }))
    }
    pub(super) fn request(&self, case: &str) -> Value {
        let probes:Vec<_>=[json!([]),json!([{"name":"fixture"}])].into_iter().map(|tools|json!({"kind":"chat","context_json":json!({"messages":[{"role":"user","content":"hello world"}],"tools":tools,"add_generation_prompt":true}).to_string()})).collect();
        let mut request = json!({"reference":REFERENCE,"original_template":TEMPLATE,"probes_json":serde_json::to_string(&probes).expect("fixed fixture serialization"),
            "specification":self.specification,"artifact_set":self.artifacts,"limits_ref":"contained-prefix-measurement-fixture/1","context_window_tokens":if case=="does-not-fit" {1} else {131072},"reserved_output_tokens":3,"reserved_additional_input_tokens":1});
        match case {
            "wrong-original" => request["original_template"] = json!(format!("{TEMPLATE}\n")),
            "unsupported-template" => request["original_template"] = json!("unsupported template"),
            "bad-assets" => request["artifact_set"] = json!("0".repeat(64)),
            "bad-input" => request["extra"] = json!("forbidden"),
            _ => {}
        }
        request
    }
    pub(super) fn inputs(&self, case: &str) -> Result<Value, RebornBuildError> {
        Ok(
            json!({"request_json":crate::prefix_generations::canonical(&self.request(case)).map_err(invalid)?}),
        )
    }
    fn report(&self, effective_template: &str) -> Value {
        // Format known outputs independently; do not call the Jinja renderer,
        // injection adapter or tokenization engine to derive expected results.
        let engineering = format!("{REFERENCE}\n\n# REQUEST CONTEXT\n");
        let user = "<|im_start|>user\nhello world\n<|im_end|>\n<|im_start|>assistant\n";
        let rendered = [
            format!("<|im_start|>system\n{engineering}\n<|im_end|>\n{user}"),
            format!(
                "<|im_start|>system\n{engineering}\n\n[{{\"name\": \"fixture\"}}]\n<|im_end|>\n{user}"
            ),
        ];
        let token_ids: Vec<Vec<u32>> = rendered
            .iter()
            .map(|text| {
                text.split_whitespace()
                    .map(|word| match word {
                        "hello" => 1,
                        "world" => 2,
                        _ => 0,
                    })
                    .collect()
            })
            .collect();
        let ids_hash = |ids: &[u32]| {
            let mut hash = Sha256::new();
            for id in ids {
                hash.update(id.to_le_bytes());
            }
            hex::encode(hash.finalize())
        };
        let common = token_ids[0]
            .iter()
            .zip(&token_ids[1])
            .take_while(|(left, right)| left == right)
            .count();
        let probes:Vec<_>=rendered.iter().zip(&token_ids).map(|(text,ids)|json!({"rendered_sha256":checksum(text.as_bytes()),"rendered_bytes":text.len(),"token_count":ids.len(),"token_ids_sha256":ids_hash(ids)})).collect();
        json!({"format":"brassclaw-model-encoding/1","tokenizer_sha256":checksum(tokenizer().as_bytes()),"tokenizer_config_sha256":checksum(config().as_bytes()),"chat_template_sha256":checksum(effective_template.as_bytes()),"truncation_disabled":true,"padding_disabled":true,"probes":probes,"common_prefix_tokens":common,"common_prefix_sha256":ids_hash(&token_ids[0][..common])})
    }
    pub(super) fn verify_observation(
        &self,
        bytes: &str,
        effective_template: &str,
        worker: &str,
    ) -> Result<Value, RebornBuildError> {
        let actual: Value = brassclaw_skills::value_contract::strict_json(
            bytes,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(invalid)?;
        if actual.as_object().is_none_or(|v| v.len() != 9)
            || actual["format"] != "brassclaw-model-encoding-observation/1"
            || actual["qualification"] != "offline-measurement-only"
            || actual["specification"] != self.specification
            || actual["artifact_set"] != self.artifacts
            || actual["template_origin"] != "supplied-template"
            || actual["model_chat_template_sha256"] != checksum(TEMPLATE.as_bytes())
            || actual["worker_checksum"] != worker
            || actual["preparation_revision"]
                .as_u64()
                .is_none_or(|n| n == 0)
            || actual["report"] != self.report(effective_template)
            || crate::prefix_generations::canonical(&actual).map_err(invalid)? != bytes
        {
            return Err(invalid(
                "closed measurement observation differs from independent oracle",
            ));
        }
        Ok(actual)
    }
    pub(super) fn comparison(
        &self,
        case: &str,
        observation: &Value,
        effective_template: &str,
    ) -> Value {
        let report = self.report(effective_template);
        let largest = report["probes"]
            .as_array()
            .expect("fixed probes")
            .iter()
            .map(|p| p["token_count"].as_u64().expect("fixed counts"))
            .max()
            .expect("fixed nonempty probes");
        let required = largest + 4;
        let context = if case == "does-not-fit" { 1u64 } else { 131072 };
        json!({"format":"brassclaw-prefix-capacity-comparison/1","qualification":"calculation-only; not approval or target verification",
            "specification":self.specification,"artifact_set":self.artifacts,"limits_ref":"contained-prefix-measurement-fixture/1","template_sha256":checksum(effective_template.as_bytes()),"worker_checksum":observation["worker_checksum"],"preparation_revision":observation["preparation_revision"],
            "context_window_tokens":context,"largest_probe_tokens":largest,"reserved_output_tokens":3,"reserved_additional_input_tokens":1,"required_tokens":required,"fits":required<=context,"remaining_tokens":context.saturating_sub(required),"shortfall_tokens":required.saturating_sub(context),"common_prefix_tokens":report["common_prefix_tokens"],"cache_qualification":"unverified","model_quality":"unmeasured"})
    }
}
