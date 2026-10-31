//! Finite independent data oracle. This is a real small tokenizer fixture, not
//! an Ornith/default-serving/cache/answer-quality qualification.
use crate::{
    RebornBuildError,
    bootstrap_encoding_fixture::{TEMPLATE, checksum, tokenizer},
    bootstrap_reply_validation::invalid,
};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

pub(super) const CASES: &[&str] = &[
    "matches",
    "duplicate",
    "changed-suffix",
    "missing-leading-prefix",
    "bad-packet",
    "denied",
];
pub(super) struct Fixture {
    pub(super) specification: String,
    pub(super) artifacts: String,
    packets: BTreeMap<String, String>,
}
impl Fixture {
    pub(super) async fn retain(pool: Arc<PgPool>) -> Result<Arc<Self>, RebornBuildError> {
        let base = crate::bootstrap_encoding_fixture::Fixture::retain(pool.clone()).await?;
        let store = crate::prefix_model_designs::PgPrefixModelDesigns::new(
            pool,
            crate::bootstrap_validation::NAMESPACE.into(),
        );
        let mut packets = BTreeMap::new();
        for case in ["matches", "changed-suffix", "missing-leading-prefix"] {
            let frame = |instructions| json!({"instructions":instructions,"evidence":["template"]});
            let first = if case == "missing-leading-prefix" {
                "world hello"
            } else {
                "hello world"
            };
            let expected = if case == "changed-suffix" {
                "hello world wrong"
            } else {
                "hello world"
            };
            let cases:Vec<_>=[("plain",first,expected),("unicode","hello ü\r\nworld","hello ü\r\nworld")].into_iter().map(|(id,content,expected)|json!({"id":id,"mode":"plain","render_context_json":json!({"messages":[{"role":"user","content":content}]}).to_string(),"expected_fixed_prefix":"hello","formatted_prompt":&expected["hello".len()..],"expected_request":expected,"evidence":["template"]})).collect();
            let document = json!({"format":"brassclaw-prefix-pair-construction-proposal/1","specification":base.specification,"artifact_set":base.artifacts,
                "rationale":"Closed installation data fixture only; not an accepted model design.",
                "evidence":[{"id":"template","file":"tokenizer_config.json","file_sha256":checksum(json!({"chat_template":TEMPLATE}).to_string().as_bytes()),"pointer":"/chat_template"},
                    {"id":"architecture","file":"config.json","file_sha256":checksum(b"{}"),"pointer":null}],
                "template":{"evidence":"template","template_sha256":checksum(TEMPLATE.as_bytes())},
                "construction":{"prefix":{"leading_literal":"","trailing_literal":"","edge_requirement":"preserve-exact","instructions":"Retain exact fixture bytes only.","evidence":["template"]},
                    "dynamic_prompt":frame("Fixture template renders the first message content verbatim."),"special_tokens":frame("Fixture uses complete original encoding, without an added special-token layer."),
                    "memory":{"instructions":"No deployed cache or memory claims are available.","evidence":["architecture"]},"supported_modes":["plain"],"unsupported_modes":{"tools":"Not qualified by this finite fixture."},
                    "join":"prepend-fixed-prefix","hard_limits_owner":"provider-definition-revision","runtime_cache":null},
                "sample_reference":"hello","worked_cases":cases});
            let proposal = store
                .retain_pair_proposal(
                    brassclaw_product_workflow::RetainPrefixDesignProposalRequest {
                        specification: base.specification.clone(),
                        proposal_bytes: document.to_string(),
                    },
                )
                .await
                .map_err(invalid)?;
            let packet = store
                .case_packet(&base.specification, &proposal.id)
                .await
                .map_err(invalid)?;
            packets.insert(case.into(), packet.case_packet_bytes);
        }
        Ok(Arc::new(Self {
            specification: base.specification,
            artifacts: base.artifacts,
            packets,
        }))
    }
    pub(super) fn packet_bytes(&self, case: &str) -> &str {
        &self.packets[if matches!(case, "duplicate" | "bad-packet" | "denied") {
            "matches"
        } else {
            case
        }]
    }
    pub(super) fn packet(&self, case: &str) -> Value {
        serde_json::from_str(self.packet_bytes(case)).expect("fixed retained packet")
    }
    pub(super) fn inputs(&self, case: &str) -> Result<Value, RebornBuildError> {
        Ok(json!({"case_packet_bytes":if case=="bad-packet" {"{}"} else {self.packet_bytes(case)}}))
    }
    pub(super) fn prepared(&self, case: &str) -> Result<Value, RebornBuildError> {
        let packet = self.packet(case);
        let mut texts = vec![json!(self.packet_bytes(case))];
        let mut probes = Vec::new();
        for case in packet["cases"]
            .as_array()
            .ok_or_else(|| invalid("fixture cases missing"))?
        {
            texts.extend([
                case["expected_render"].clone(),
                case["context_json"].clone(),
                case["expected_prefix"].clone(),
            ]);
            probes.push(json!({"kind":"leading_prefix_chat","context_json":case["context_json"],"leading_prefix":case["expected_prefix"]}));
            probes.push(json!({"kind":"text","text":case["expected_render"]}));
        }
        Ok(
            json!({"bank_bytes":self.packet_bytes(case),"texts":texts,"probes_json":crate::prefix_generations::canonical(&json!(probes)).map_err(invalid)?,"specification":self.specification,"artifact_set":self.artifacts}),
        )
    }
    fn ids(values: &[u32]) -> String {
        let mut hash = Sha256::new();
        for id in values {
            hash.update(id.to_le_bytes());
        }
        hex::encode(hash.finalize())
    }
    fn report(&self, case: &str) -> Value {
        // Expected full renders and vocabulary IDs are specified independently;
        // never invoke Jinja, Monty, tokenizer code or the comparison to obtain them.
        let packet = self.packet(case);
        let mut probes = Vec::new();
        for (index, (rendered, ids, prompt)) in [
            ("hello world", vec![1, 2], vec![2]),
            ("hello ü\r\nworld", vec![1, 0, 2], vec![0, 2]),
        ]
        .into_iter()
        .enumerate()
        {
            let source = &packet["cases"][index];
            let probe = |text: &str, ids: &[u32]| json!({"rendered_sha256":checksum(text.as_bytes()),"rendered_bytes":text.len(),"token_count":ids.len(),"token_ids_sha256":Self::ids(ids)});
            let mut rendered_probe = probe(rendered, &ids);
            rendered_probe["partition"] = json!({"prefix_start":0,"prefix_end":5,"prefix_bytes_sha256":checksum(b"hello"),"prefix_tokens":1,"prompt_tokens":prompt.len(),"prefix_token_ids_sha256":Self::ids(&[1]),"prompt_token_ids_sha256":Self::ids(&prompt),"qualification":"verified-leading-render-bytes-only","render_context_sha256":checksum(source["context_json"].as_str().expect("fixed context").as_bytes())});
            probes.push(rendered_probe);
            let expected = source["expected_render"].as_str().expect("fixed literal");
            probes.push(probe(
                expected,
                &if index == 0 && case == "changed-suffix" {
                    vec![1, 2, 0]
                } else {
                    ids
                },
            ));
        }
        json!({"format":"brassclaw-model-encoding/2","tokenizer_sha256":checksum(tokenizer().as_bytes()),"tokenizer_config_sha256":checksum(json!({"chat_template":TEMPLATE}).to_string().as_bytes()),"chat_template_sha256":checksum(TEMPLATE.as_bytes()),"truncation_disabled":true,"padding_disabled":true,"probes":probes,"common_prefix_tokens":1,"common_prefix_sha256":Self::ids(&[1])})
    }
    pub(super) fn verify_observation(
        &self,
        case: &str,
        bytes: &str,
        worker: &str,
    ) -> Result<Value, RebornBuildError> {
        let actual = brassclaw_skills::value_contract::strict_json(
            bytes,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(invalid)?;
        if actual.as_object().is_none_or(|o| o.len() != 9)
            || actual["format"] != "brassclaw-model-encoding-observation/1"
            || actual["specification"] != self.specification
            || actual["artifact_set"] != self.artifacts
            || actual["template_origin"] != "model-artifact-template"
            || actual["model_chat_template_sha256"] != checksum(TEMPLATE.as_bytes())
            || actual["worker_checksum"] != worker
            || actual["preparation_revision"]
                .as_u64()
                .is_none_or(|r| r == 0)
            || actual["qualification"] != "offline-measurement-only"
            || actual["report"] != self.report(case)
            || crate::prefix_generations::canonical(&actual).map_err(invalid)? != bytes
        {
            return Err(invalid(
                "closed pair encoding differs from independent oracle",
            ));
        }
        Ok(actual)
    }
    pub(super) fn comparison(&self, case: &str, observed: &Value) -> Value {
        let packet = self.packet(case);
        let report = self.report(case);
        let cases:Vec<_>=packet["cases"].as_array().expect("fixed cases").iter().enumerate().map(|(index,source)|{
            let native=&report["probes"][2*index];let partition=&native["partition"];
            json!({"id":source["id"],"mode":source["mode"],"matches":index!=0 || case!="changed-suffix","expected_rendered_sha256":checksum(source["expected_render"].as_str().expect("fixed literal").as_bytes()),"rendered_sha256":native["rendered_sha256"],"token_ids_sha256":native["token_ids_sha256"],"prefix_token_ids_sha256":partition["prefix_token_ids_sha256"],"prefix_tokens":partition["prefix_tokens"],"prompt_tokens":partition["prompt_tokens"],"total_tokens":native["token_count"]})
        }).collect();
        json!({"format":"brassclaw-prefix-pair-case-comparison/1","case_packet_sha256":checksum(self.packet_bytes(case).as_bytes()),"proposal":packet["proposal"],"specification":self.specification,"artifact_set":self.artifacts,"template_sha256":checksum(TEMPLATE.as_bytes()),"worker_checksum":observed["worker_checksum"],"preparation_revision":observed["preparation_revision"],"cases":cases,"all_cases_match":case!="changed-suffix","stable_prefix_encoding":true,"qualification":"retained-template-cases-only","provider_wire_qualified":false,"context_fit":null,"cache_quality":"unmeasured","model_quality":"unmeasured"})
    }
}
