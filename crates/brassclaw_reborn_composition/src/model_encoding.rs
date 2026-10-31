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
#[cfg(test)]
use brassclaw_model_encoding::PROTOCOL;
use brassclaw_model_encoding::{
    Assets, EncodingReport, Probe, ProbeEncoding, Request, checksum, parse_probes, report_protocol,
    request_protocol,
};
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

// Pin the caller's exact physical span before moving input to the contained
// worker. This is consistency checking, not semantic prefix qualification.
enum PartitionRenderExpectation {
    Literal {
        hash: String,
        bytes: u64,
    },
    Chat {
        context_hash: String,
    },
    Wire {
        request_hash: String,
        mapping_hash: String,
        model: String,
    },
}
struct PartitionExpectation {
    start: usize,
    end: usize,
    bytes_hash: String,
    rendering: PartitionRenderExpectation,
}
impl PartitionExpectation {
    fn capture(probe: &Probe) -> Result<Option<Self>, RuntimeDispatchErrorKind> {
        match probe {
            Probe::PartitionedText {
                text,
                prefix_start,
                prefix_end,
            } => {
                let bytes = text
                    .get(*prefix_start..*prefix_end)
                    .filter(|_| prefix_start != prefix_end || *prefix_start == 0)
                    .ok_or(RuntimeDispatchErrorKind::InputEncode)?;
                Ok(Some(Self {
                    start: *prefix_start,
                    end: *prefix_end,
                    bytes_hash: checksum(bytes.as_bytes()),
                    rendering: PartitionRenderExpectation::Literal {
                        hash: checksum(text.as_bytes()),
                        bytes: text.len() as u64,
                    },
                }))
            }
            Probe::LeadingPrefixChat {
                context_json,
                leading_prefix,
            } => Ok(Some(Self {
                start: 0,
                end: leading_prefix.len(),
                bytes_hash: checksum(leading_prefix.as_bytes()),
                rendering: PartitionRenderExpectation::Chat {
                    context_hash: checksum(context_json.as_bytes()),
                },
            })),
            Probe::WireChat {
                request_json,
                mapping_json,
                expected_model,
                leading_prefix,
            } => Ok(Some(Self {
                start: 0,
                end: leading_prefix.len(),
                bytes_hash: checksum(leading_prefix.as_bytes()),
                rendering: PartitionRenderExpectation::Wire {
                    request_hash: checksum(request_json.as_bytes()),
                    mapping_hash: checksum(mapping_json.as_bytes()),
                    model: expected_model.clone(),
                },
            })),
            Probe::Text { .. } | Probe::Chat { .. } => Ok(None),
        }
    }
    fn matches(&self, probe: &ProbeEncoding) -> bool {
        let Some(partition) = &probe.partition else {
            return false;
        };
        let hash = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let rendering_matches = match &self.rendering {
            PartitionRenderExpectation::Literal { hash, bytes } => {
                probe.wire_input.is_none()
                    && probe.rendered_sha256 == *hash
                    && probe.rendered_bytes == *bytes
                    && partition.qualification == "supplied-byte-span-only"
                    && partition.render_context_sha256.is_none()
            }
            PartitionRenderExpectation::Chat { context_hash } => {
                probe.wire_input.is_none()
                    && probe.rendered_bytes >= self.end as u64
                    && hash(&probe.rendered_sha256)
                    && partition.qualification == "verified-leading-render-bytes-only"
                    && partition.render_context_sha256.as_ref() == Some(context_hash)
            }
            PartitionRenderExpectation::Wire {
                request_hash,
                mapping_hash,
                model,
            } => {
                probe.rendered_bytes >= self.end as u64
                    && hash(&probe.rendered_sha256)
                    && partition.qualification == "verified-leading-render-bytes-only"
                    && probe.wire_input.as_ref().is_some_and(|wire| {
                        wire.request_sha256 == *request_hash
                            && wire.mapping_sha256 == *mapping_hash
                            && wire.model == *model
                            && wire.reserved_output_tokens > 0
                            && hash(&wire.render_context_sha256)
                            && partition.render_context_sha256.as_ref()
                                == Some(&wire.render_context_sha256)
                    })
            }
        };
        rendering_matches
            && partition.prefix_start == self.start
            && partition.prefix_end == self.end
            && partition.prefix_bytes_sha256 == self.bytes_hash
            && partition.prefix_tokens.checked_add(partition.prompt_tokens)
                == Some(probe.token_count)
            && (self.start != self.end || partition.prefix_tokens == 0)
            && hash(&partition.prefix_token_ids_sha256)
            && hash(&partition.prompt_token_ids_sha256)
    }
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
        let request_format = request_protocol(&probes);
        let expected_report_format = report_protocol(&probes);
        let partitions = probes
            .iter()
            .map(PartitionExpectation::capture)
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?;
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
                format: request_format.into(),
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
        if report.format != expected_report_format
            || !report.truncation_disabled
            || !report.padding_disabled
            || report.probes.len() != count
            || report
                .probes
                .iter()
                .zip(&partitions)
                .any(|(probe, expected)| match expected {
                    Some(expected) => !expected.matches(probe),
                    None => probe.partition.is_some() || probe.wire_input.is_some(),
                })
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
    #[cfg(feature = "root-llm-provider")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_partition_qualification_uses_actual_worker_recipes_and_durable_journal() {
        use brassclaw_host_api::runtime_policy::*;
        let rig = crate::runtime::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let directory = tempfile::tempdir().unwrap();
        let runtime = Box::pin(crate::build_reborn_runtime(
            crate::RebornRuntimeInput::from_services(
                rig.build_input("token-partition-owner", directory.path())
                    .with_runtime_policy(EffectiveRuntimePolicy {
                        deployment: DeploymentMode::LocalSingleUser,
                        requested_profile: RuntimeProfile::LocalDev,
                        resolved_profile: RuntimeProfile::LocalDev,
                        filesystem_backend: FilesystemBackendKind::HostWorkspace,
                        process_backend: ProcessBackendKind::LocalHost,
                        network_mode: NetworkMode::DirectLogged,
                        secret_mode: SecretMode::ScrubbedEnv,
                        approval_policy: ApprovalPolicy::AskDestructive,
                        audit_mode: AuditMode::LocalMinimal,
                    }),
            ),
        ))
        .await
        .unwrap();
        // Readiness executes both exact packaged encoding Recipes, including
        // physical partition success and boundary/UTF-8 rejections, through the
        // real contained worker. This verifies the retained native answers and
        // rejects altered/missing cases against their actual durable journal.
        let catalogue = runtime.installed_catalogue.clone();
        crate::bootstrap_encoding_approval::assert_retained_proof_checks_actual_journal(
            catalogue.pool.clone(),
            &catalogue.encoding_approvals,
        )
        .await;
    }
    #[test]
    fn partition_report_is_bound_to_the_exact_supplied_text_and_range() {
        let probes = vec![Probe::PartitionedText {
            text: "hello world".into(),
            prefix_start: 6,
            prefix_end: 11,
        }];
        let expected = PartitionExpectation::capture(&probes[0]).unwrap().unwrap();
        let report = brassclaw_model_encoding::analyze(Request {
            format: request_protocol(&probes).into(),
            assets: Assets {
                tokenizer_json: crate::bootstrap_encoding_fixture::tokenizer(),
                tokenizer_config_json: "{}".into(),
                chat_template: "{{ messages }}".into(),
            },
            probes,
        })
        .unwrap();
        assert!(expected.matches(&report.probes[0]));
        let original = serde_json::to_value(&report.probes[0]).unwrap();
        for (field, value) in [
            ("prefix_start", json!(0)),
            ("prefix_end", json!(10)),
            ("prefix_bytes_sha256", json!("0".repeat(64))),
            ("prefix_tokens", json!(u64::MAX)),
            ("qualification", json!("semantic-prefix")),
            ("prompt_token_ids_sha256", json!("invalid")),
        ] {
            let mut changed = original.clone();
            changed["partition"][field] = value;
            assert!(
                !expected.matches(&serde_json::from_value(changed).unwrap()),
                "{field}"
            );
        }
        let mut changed = original;
        changed["rendered_sha256"] = json!("0".repeat(64));
        assert!(!expected.matches(&serde_json::from_value(changed).unwrap()));
        assert!(
            PartitionExpectation::capture(&Probe::PartitionedText {
                text: "ü".into(),
                prefix_start: 0,
                prefix_end: 1
            })
            .is_err()
        );
    }
    #[test]
    fn leading_report_binds_ordered_context_and_cannot_claim_literal_span_qualification() {
        let context = r#"{"messages":[{"role":"system","content":"hello world"}]}"#;
        let probes = vec![Probe::LeadingPrefixChat {
            context_json: context.into(),
            leading_prefix: "hello".into(),
        }];
        let expected = PartitionExpectation::capture(&probes[0]).unwrap().unwrap();
        let report = brassclaw_model_encoding::analyze(Request {
            format: request_protocol(&probes).into(),
            assets: Assets {
                tokenizer_json: crate::bootstrap_encoding_fixture::tokenizer(),
                tokenizer_config_json: "{}".into(),
                chat_template: "{{ messages[0].content }}".into(),
            },
            probes,
        })
        .unwrap();
        let probe = &report.probes[0];
        assert!(expected.matches(probe));
        let original = serde_json::to_value(probe).unwrap();
        for (field, value) in [
            ("render_context_sha256", json!("0".repeat(64))),
            ("qualification", json!("supplied-byte-span-only")),
            ("prefix_start", json!(1)),
            ("prefix_end", json!(6)),
            ("prefix_bytes_sha256", json!("0".repeat(64))),
        ] {
            let mut changed = original.clone();
            changed["partition"][field] = value;
            assert!(
                !expected.matches(&serde_json::from_value(changed).unwrap()),
                "{field}"
            );
        }
        let mut missing = original.clone();
        missing["partition"]
            .as_object_mut()
            .unwrap()
            .remove("render_context_sha256");
        assert!(!expected.matches(&serde_json::from_value(missing).unwrap()));
        let mut short = original;
        short["rendered_bytes"] = json!(0);
        assert!(!expected.matches(&serde_json::from_value(short).unwrap()));
        let literal = PartitionExpectation::capture(&Probe::PartitionedText {
            text: "hello world".into(),
            prefix_start: 0,
            prefix_end: 5,
        })
        .unwrap()
        .unwrap();
        assert!(!literal.matches(probe));
    }
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
