//! Mechanical offline model encoding over the existing immutable artifact store
//! and retained preparation worker. No design, source, acceptance or activation
//! decisions, new provider pool, mutable checkout files or remote model code.
use crate::source_inspection::ContainedSourceInspection;
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_host_api::{ResourceScope, ResourceUsage, RuntimeDispatchErrorKind};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityResult, ModelEncodingBackend,
};
use brassclaw_model_encoding::{Assets, EncodingReport, PROTOCOL, Request, checksum, parse_probes};
use brassclaw_monty_host::{
    VmFailure,
    process::ProcessFailure,
    utility::{self, UtilityOutput, UtilityRequest},
};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};

pub(crate) struct ContainedModelEncoding {
    pool: Arc<PgPool>,
    worker: Arc<ContainedSourceInspection>,
}
fn failure(kind: RuntimeDispatchErrorKind) -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(kind)
}
impl ContainedModelEncoding {
    pub(crate) fn new(pool: Arc<PgPool>, worker: Arc<ContainedSourceInspection>) -> Self {
        Self { pool, worker }
    }
}
// Count escaped JSON before allocating another complete asset/probe payload.
// The utility's outer framed codec independently bounds its additional escaping.
fn encode_request(request: &Request, limit: usize) -> Result<String, RuntimeDispatchErrorKind> {
    let limit = limit.min(brassclaw_model_encoding::MAX_INPUT_BYTES);
    brassclaw_common::json::count_json_bytes(request, Some(limit as u64))
        .map_err(|_| RuntimeDispatchErrorKind::Resource)?;
    serde_json::to_string(request).map_err(|_| RuntimeDispatchErrorKind::InputEncode)
}

#[async_trait]
impl ModelEncodingBackend for ContainedModelEncoding {
    async fn encode(
        &self,
        specification: &str,
        artifact_set: &str,
        probes_json: &str,
        template_utf8: Option<&str>,
        scope: &ResourceScope,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let prepared = self.worker.prepare().await?;
        if probes_json.len() > prepared.limits.bounds.max_value_bytes {
            return Err(failure(RuntimeDispatchErrorKind::Resource));
        }
        let probes = parse_probes(probes_json).map_err(|error| {
            failure(match error {
                brassclaw_model_encoding::EncodingError::Capacity => {
                    RuntimeDispatchErrorKind::Resource
                }
                _ => RuntimeDispatchErrorKind::InputEncode,
            })
        })?;
        let count = probes.len();
        let assets = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
            self.pool.clone(),
            scope.tenant_id.as_str().to_owned(),
        )
        .get(specification, artifact_set)
        .await
        .map_err(|error| {
            failure(match error {
                brassclaw_product_workflow::InterceptorConfigServiceError::InvalidRequest {
                    ..
                } => RuntimeDispatchErrorKind::InputEncode,
                _ => RuntimeDispatchErrorKind::OperationFailed,
            })
        })?;
        // get() verified the entire set, inventory and exact specification binding.
        let package: Value = brassclaw_skills::value_contract::strict_json(
            &assets.artifact_set_bytes,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(|_| failure(RuntimeDispatchErrorKind::OperationFailed))?;
        let decode = |name: &str| -> Result<String, FirstPartyCapabilityError> {
            let encoded = package["artifacts"][name]["data"]
                .as_str()
                .ok_or_else(|| failure(RuntimeDispatchErrorKind::InputEncode))?;
            String::from_utf8(
                STANDARD
                    .decode(encoded)
                    .map_err(|_| failure(RuntimeDispatchErrorKind::OperationFailed))?,
            )
            .map_err(|_| failure(RuntimeDispatchErrorKind::OperationFailed))
        };
        let tokenizer_json = decode("tokenizer.json")?;
        let tokenizer_config_json = decode("tokenizer_config.json")?;
        let chat_template = if package["artifacts"].get("chat_template.jinja").is_some() {
            decode("chat_template.jinja")?
        } else {
            let config: Value = brassclaw_skills::value_contract::strict_json(
                &tokenizer_config_json,
                brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
            )
            .map_err(|_| failure(RuntimeDispatchErrorKind::InputEncode))?;
            config["chat_template"]
                .as_str()
                .ok_or_else(|| failure(RuntimeDispatchErrorKind::InputEncode))?
                .to_owned()
        };
        let original_template_sha256 = checksum(chat_template.as_bytes());
        let template_origin = if template_utf8.is_some() {
            "supplied-template"
        } else {
            "model-artifact-template"
        };
        let chat_template = template_utf8.map(str::to_owned).unwrap_or(chat_template);
        let expected = [
            checksum(tokenizer_json.as_bytes()),
            checksum(tokenizer_config_json.as_bytes()),
            checksum(chat_template.as_bytes()),
        ];
        drop(package);
        drop(assets);
        let request_bytes = encode_request(
            &Request {
                format: PROTOCOL.into(),
                assets: Assets {
                    tokenizer_json,
                    tokenizer_config_json,
                    chat_template,
                },
                probes,
            },
            prepared
                .limits
                .bounds
                .max_value_bytes
                .min(prepared.limits.process.max_frame_bytes),
        )
        .map_err(failure)?;
        let started = Instant::now();
        let encoded = utility::execute(
            &prepared.worker.path(),
            UtilityRequest::EncodeModel {
                request_bytes,
                bounds: prepared.limits.bounds,
            },
            prepared.limits.process,
        )
        .await;
        let usage = ResourceUsage {
            wall_clock_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            process_count: 1,
            ..ResourceUsage::default()
        };
        let encoded = encoded.map_err(|error| {
            let kind = match error.kind {
                ProcessFailure::ValueLimit
                | ProcessFailure::FrameLimit
                | ProcessFailure::Deadline
                | ProcessFailure::Vm(VmFailure::ResourceLimit) => {
                    RuntimeDispatchErrorKind::Resource
                }
                ProcessFailure::Vm(VmFailure::InvalidInputs) => {
                    RuntimeDispatchErrorKind::InputEncode
                }
                _ => RuntimeDispatchErrorKind::OperationFailed,
            };
            let mut usage = usage.clone();
            if matches!(
                error.kind,
                ProcessFailure::InvalidLimits
                    | ProcessFailure::InvalidExecutable
                    | ProcessFailure::ValueLimit
                    | ProcessFailure::FrameLimit
                    | ProcessFailure::Spawn
            ) {
                usage.process_count = 0;
            }
            failure(kind).with_usage(usage)
        })?;
        let report_bytes = match encoded {
            UtilityOutput::EncodedModel { report_bytes } => report_bytes,
            UtilityOutput::ModelEncodingFailed { failure: reason } => {
                return Err(FirstPartyCapabilityError::with_safe_summary(
                    if matches!(reason, brassclaw_model_encoding::EncodingError::Capacity) {
                        RuntimeDispatchErrorKind::Resource
                    } else {
                        RuntimeDispatchErrorKind::InputEncode
                    },
                    reason.to_string(),
                )
                .with_usage(usage));
            }
            _ => return Err(failure(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage)),
        };
        let report: EncodingReport = serde_json::from_str(&report_bytes).map_err(|_| {
            failure(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage.clone())
        })?;
        if report.format != "brassclaw-model-encoding/1"
            || !report.truncation_disabled
            || !report.padding_disabled
            || report.probes.len() != count
            || [
                report.tokenizer_sha256.as_str(),
                report.tokenizer_config_sha256.as_str(),
                report.chat_template_sha256.as_str(),
            ] != [
                expected[0].as_str(),
                expected[1].as_str(),
                expected[2].as_str(),
            ]
            || report
                .probes
                .iter()
                .any(|probe| report.common_prefix_tokens > probe.token_count)
        {
            return Err(failure(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage));
        }
        let output = json!({"format":"brassclaw-model-encoding-observation/1","specification":specification,"artifact_set":artifact_set,
            "preparation_revision":prepared.revision,"model_chat_template_sha256":original_template_sha256,"template_origin":template_origin,"worker_checksum":hex::encode(prepared.worker.checksum()),"report":report,
            "qualification":"offline-measurement-only"});
        let bytes = crate::prefix_generations::canonical(&output).map_err(|_| {
            failure(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage.clone())
        })?;
        let value = Value::String(bytes);
        let encoded = serde_json::to_vec(&value).map_err(|_| {
            failure(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage.clone())
        })?;
        if encoded.len() > prepared.limits.bounds.max_value_bytes
            || encoded.len() > prepared.limits.process.max_frame_bytes
        {
            return Err(failure(RuntimeDispatchErrorKind::Resource).with_usage(usage));
        }
        Ok(FirstPartyCapabilityResult::new(
            value,
            ResourceUsage {
                output_bytes: encoded.len() as u64,
                ..usage
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoding_payload_capacity_counts_escaped_bytes_before_allocation() {
        let request = Request {
            format: PROTOCOL.into(),
            assets: Assets {
                tokenizer_json: "{}".into(),
                tokenizer_config_json: "{}".into(),
                chat_template: "{{ messages }}".into(),
            },
            probes: vec![brassclaw_model_encoding::Probe::Text {
                text: "\0".repeat(256),
            }],
        };
        let exact = serde_json::to_string(&request).unwrap();
        assert!(exact.len() > 256 * 6);
        assert!(matches!(
            encode_request(&request, 0),
            Err(RuntimeDispatchErrorKind::Resource)
        ));
        assert!(matches!(
            encode_request(&request, exact.len() - 1),
            Err(RuntimeDispatchErrorKind::Resource)
        ));
        assert_eq!(encode_request(&request, exact.len()).unwrap(), exact);
    }
}
