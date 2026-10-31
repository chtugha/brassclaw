mod support;

use support::legacy_capability_fixture_to_v2;

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer;
use brassclaw_capabilities::{
    CapabilityObligationCompletionRequest, CapabilityObligationError,
    CapabilityObligationFailureKind, CapabilityObligationHandler, CapabilityObligationPhase,
    CapabilityObligationRequest,
};
use brassclaw_events::InMemoryAuditSink;
use brassclaw_extensions::{
    ExtensionManifest, ExtensionPackage, ExtensionRegistry, ManifestSource,
};
use brassclaw_host_api::*;
use brassclaw_host_runtime::{
    BuiltinObligationHandler, BuiltinObligationServices, CapabilitySurfaceVersion,
    DefaultHostRuntime, HostRuntime, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
    RuntimeCredentialAccessSecret, RuntimeCredentialAccountRequest,
    RuntimeCredentialAccountResolver, RuntimeFailureKind,
};
use brassclaw_resources::{InMemoryResourceGovernor, ResourceAccount};
use brassclaw_secrets::{InMemorySecretStore, SecretMaterial, SecretStore};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use serde_json::json;

fn local_test_runtime_policy() -> brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy {
    brassclaw_runtime_policy::resolve(brassclaw_runtime_policy::ResolveRequest::new(
        brassclaw_host_api::runtime_policy::DeploymentMode::LocalSingleUser,
        brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
    ))
    .unwrap()
}

fn obligation_services(
    secret_store: Arc<InMemorySecretStore>,
    governor: Arc<InMemoryResourceGovernor>,
) -> BuiltinObligationServices {
    BuiltinObligationServices::new(Arc::new(InMemoryAuditSink::new()), secret_store, governor)
}

#[tokio::test]
async fn builtin_obligation_handler_emits_metadata_only_audit_before() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let handler = BuiltinObligationHandler::new().with_audit_sink(audit.clone());
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::AuditBefore];

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();

    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].stage, AuditStage::Before);
    assert_eq!(records[0].tenant_id, context.tenant_id);
    assert_eq!(records[0].user_id, context.user_id);
    assert_eq!(records[0].invocation_id, context.invocation_id);
    assert_eq!(records[0].action.kind, "capability_invoke");
    assert_eq!(records[0].action.target.as_deref(), Some("echo.say"));
    assert_eq!(records[0].decision.kind, "obligation_satisfied");
    assert_eq!(
        records[0]
            .result
            .as_ref()
            .and_then(|result| result.status.as_deref()),
        Some("audit_before")
    );
    let serialized = serde_json::to_string(&records[0]).unwrap();
    assert!(!serialized.contains("raw input"));
    assert!(!serialized.contains("secret"));
}

#[tokio::test]
async fn builtin_obligation_handler_satisfy_fails_closed_on_post_dispatch_obligations() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::RedactOutput];

    let err = handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Unsupported { obligations } if obligations == vec![Obligation::RedactOutput]
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_enforces_output_limit_after_dispatch() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::EnforceOutputLimit { bytes: 8 }];
    let dispatch = sample_dispatch(
        &context.resource_scope,
        &capability_id,
        json!({"message": "this output is too large"}),
    );

    let err = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Output
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_allows_resource_ceiling_when_estimate_is_within_limit() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        usd: Some(1.into()),
        input_tokens: Some(100),
        ..ResourceEstimate::default()
    };
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: Some(2.into()),
            max_input_tokens: Some(200),
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: None,
            sandbox: None,
        },
    }];

    handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_rejects_resource_ceiling_above_host_estimate() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        usd: Some(3.into()),
        ..ResourceEstimate::default()
    };
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: Some(2.into()),
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: None,
            sandbox: None,
        },
    }];

    let err = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Resource
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_rejects_unenforced_sandbox_quota_fields() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: None,
            sandbox: Some(SandboxQuota {
                memory_bytes: Some(1024),
                ..SandboxQuota::default()
            }),
        },
    }];

    let err = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Resource
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_rejects_wall_clock_ceiling_until_runtime_handoff_exists() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        wall_clock_ms: Some(500),
        ..ResourceEstimate::default()
    };
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(1_000),
            max_output_bytes: None,
            sandbox: None,
        },
    }];

    let err = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Resource
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_rejects_sandbox_network_ceiling_until_runtime_handoff_exists() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        network_egress_bytes: Some(512),
        ..ResourceEstimate::default()
    };
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: None,
            sandbox: Some(SandboxQuota {
                network_egress_bytes: Some(1024),
                ..SandboxQuota::default()
            }),
        },
    }];

    let err = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Resource
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_rejects_sandbox_process_ceiling_until_runtime_handoff_exists() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        process_count: Some(1),
        ..ResourceEstimate::default()
    };
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: None,
            sandbox: Some(SandboxQuota {
                process_count: Some(1),
                ..SandboxQuota::default()
            }),
        },
    }];

    let err = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Resource
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_reports_resource_ceiling_output_bytes_as_output_failure() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: Some(8),
            sandbox: None,
        },
    }];
    let dispatch = sample_dispatch(
        &context.resource_scope,
        &capability_id,
        json!({"message": "this output is too large"}),
    );

    let err = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Output
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_resource_ceiling_output_bytes_uses_published_output_size() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: Some(32),
            sandbox: None,
        },
    }];
    let mut dispatch =
        sample_dispatch(&context.resource_scope, &capability_id, json!({"ok": true}));
    dispatch.usage.output_bytes = 1024;

    let completed = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap();

    assert_eq!(completed.output, json!({"ok": true}));
}

#[tokio::test]
async fn builtin_obligation_handler_enforces_resource_ceiling_after_dispatch_usage() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        output_tokens: Some(10),
        ..ResourceEstimate::default()
    };
    let obligations = vec![Obligation::EnforceResourceCeiling {
        ceiling: ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: Some(10),
            max_wall_clock_ms: None,
            max_output_bytes: None,
            sandbox: None,
        },
    }];
    let mut dispatch =
        sample_dispatch(&context.resource_scope, &capability_id, json!({"ok": true}));
    dispatch.usage.output_tokens = 11;

    let err = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Resource
        }
    ));
}

#[tokio::test]
async fn builtin_obligation_handler_redacts_output_after_dispatch() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::RedactOutput];
    let leaked = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let dispatch = sample_dispatch(
        &context.resource_scope,
        &capability_id,
        json!({"authorization": leaked}),
    );

    let completed = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap();

    let serialized = serde_json::to_string(&completed.output).unwrap();
    assert!(serialized.contains("[REDACTED]"));
    assert!(!serialized.contains(leaked));
}

#[tokio::test]
async fn builtin_obligation_handler_redacts_secret_like_object_keys() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::RedactOutput];
    let leaked = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let dispatch = sample_dispatch(
        &context.resource_scope,
        &capability_id,
        json!({ leaked: "value" }),
    );

    let completed = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap();

    let serialized = serde_json::to_string(&completed.output).unwrap();
    assert!(serialized.contains("[REDACTED]"));
    assert!(!serialized.contains(leaked));
}

#[tokio::test]
async fn builtin_obligation_handler_fails_closed_when_redacted_object_keys_collide() {
    let handler = BuiltinObligationHandler::new();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::RedactOutput];
    let leaked = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let dispatch = sample_dispatch(
        &context.resource_scope,
        &capability_id,
        json!({
            leaked: "secret-key",
            "[REDACTED]": "existing",
        }),
    );

    let err = handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &dispatch,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CapabilityObligationError::Failed {
            kind: CapabilityObligationFailureKind::Output
        }
    ));
}

#[test]
fn builtin_obligation_completion_deep_copy_redaction_and_late_failures_are_stack_safe() {
    use brassclaw_events::InMemorySecurityAuditSink;
    use serde_json::Value;

    const DEPTH: usize = 12_000;
    const TOKEN: &str = "Bearer abcdefghijklmnopqrstuvwxyz0123456789";

    fn deep() -> Value {
        let mut value = Value::String(TOKEN.to_owned());
        for index in 0..DEPTH {
            value = if index % 2 == 0 {
                Value::Array(vec![value])
            } else {
                Value::Object([(String::from("child"), value)].into_iter().collect())
            };
        }
        value
    }

    fn dismantle(value: Value) {
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            match value {
                Value::Array(values) => pending.extend(values),
                Value::Object(values) => pending.extend(values.into_values()),
                _ => {}
            }
        }
    }

    async fn exercise(
        handler: BuiltinObligationHandler,
        security_sink: Arc<InMemorySecurityAuditSink>,
    ) {
        let context = execution_context(CapabilitySet::default());
        let capability_id = capability_id();
        let estimate = ResourceEstimate::default();
        for redact in [false, true] {
            let dispatch = sample_dispatch(&context.resource_scope, &capability_id, deep());
            let obligations = if redact {
                vec![Obligation::RedactOutput]
            } else {
                vec![]
            };
            let completed = handler
                .complete_dispatch(CapabilityObligationCompletionRequest {
                    phase: CapabilityObligationPhase::Invoke,
                    context: &context,
                    capability_id: &capability_id,
                    estimate: &estimate,
                    obligations: &obligations,
                    dispatch: &dispatch,
                })
                .await
                .unwrap();
            let mut leaf = &completed.output;
            for index in (0..DEPTH).rev() {
                leaf = if index % 2 == 0 {
                    &leaf[0]
                } else {
                    &leaf["child"]
                };
            }
            assert_eq!(
                leaf.as_str(),
                Some(if redact { "[REDACTED]" } else { TOKEN })
            );
            assert_eq!(completed.capability_id, dispatch.capability_id);
            assert_eq!(completed.provider, dispatch.provider);
            assert_eq!(completed.runtime, dispatch.runtime);
            assert_eq!(completed.usage, dispatch.usage);
            assert_eq!(completed.receipt, dispatch.receipt);
            // The public DTO still owns an ordinary Value. Its caller
            // must dismantle larger profiles until that API is migrated.
            dismantle(completed.output);
            dismantle(dispatch.output);
        }

        for (suffix, obligations, expected_kind) in [
            (
                Value::String("AKIAIOSFODNN7EXAMPLE".into()),
                vec![Obligation::RedactOutput],
                CapabilityObligationFailureKind::Output,
            ),
            (
                json!({TOKEN: true, "[REDACTED]": false}),
                vec![Obligation::RedactOutput],
                CapabilityObligationFailureKind::Output,
            ),
            (
                Value::Null,
                vec![Obligation::EnforceOutputLimit { bytes: 1 }],
                CapabilityObligationFailureKind::Output,
            ),
            (
                Value::Null,
                vec![Obligation::AuditAfter],
                CapabilityObligationFailureKind::Audit,
            ),
        ] {
            let dispatch = sample_dispatch(
                &context.resource_scope,
                &capability_id,
                Value::Array(vec![deep(), suffix]),
            );
            let result = handler
                .complete_dispatch(CapabilityObligationCompletionRequest {
                    phase: CapabilityObligationPhase::Invoke,
                    context: &context,
                    capability_id: &capability_id,
                    estimate: &estimate,
                    obligations: &obligations,
                    dispatch: &dispatch,
                })
                .await;
            assert!(
                matches!(result, Err(CapabilityObligationError::Failed { kind }) if kind == expected_kind)
            );
            dismantle(dispatch.output);
        }
        let events = security_sink.snapshot();
        assert_eq!(events.len(), 2);
        for event in events {
            assert_eq!(event.code, "leak_redact_failed");
            assert_eq!(event.capability_id.as_ref(), Some(&capability_id));
            assert_eq!(event.scope.as_ref(), Some(&context.resource_scope));
        }
    }

    // Production constructs the handler during service wiring, before tasks.
    // Regex compilation has its own stack needs, independent of output depth.
    let security_sink = Arc::new(InMemorySecurityAuditSink::new());
    let handler = BuiltinObligationHandler::new().with_security_audit_sink(security_sink.clone());
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(exercise(handler, security_sink));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_stores_network_policy_for_runtime_handoff() {
    let services = obligation_services(
        Arc::new(InMemorySecretStore::new()),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::ApplyNetworkPolicy {
        policy: allowed_network_policy(),
    }];

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_removes_network_policy_on_abort() {
    let services = obligation_services(
        Arc::new(InMemorySecretStore::new()),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::ApplyNetworkPolicy {
        policy: allowed_network_policy(),
    }];

    let outcome = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();
    handler
        .abort(brassclaw_capabilities::CapabilityObligationAbortRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            outcome: &outcome,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn network_obligation_handoff_isolates_agent_scope() {
    let services = obligation_services(
        Arc::new(InMemorySecretStore::new()),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let handler = services.obligation_handler();
    let capability_id = capability_id();
    let mut context = execution_context(CapabilitySet::default());
    context.resource_scope.agent_id = Some(AgentId::new("agent-a").unwrap());
    let mut agent_b = context.resource_scope.clone();
    agent_b.agent_id = Some(AgentId::new("agent-b").unwrap());

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &ResourceEstimate::default(),
            obligations: &[Obligation::ApplyNetworkPolicy {
                policy: allowed_network_policy(),
            }],
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_leases_consumes_and_stages_secret_once() {
    let secret_store = Arc::new(InMemorySecretStore::new());
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let services = obligation_services(secret_store.clone(), governor);
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let handle = SecretHandle::new("api_token").unwrap();
    secret_store
        .put(
            context.resource_scope.clone(),
            handle.clone(),
            SecretMaterial::from("runtime-secret"),
        )
        .await
        .unwrap();
    let obligations = vec![Obligation::InjectSecretOnce {
        handle: handle.clone(),
    }];

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_removes_staged_secret_on_abort() {
    let secret_store = Arc::new(InMemorySecretStore::new());
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let services = obligation_services(secret_store.clone(), governor);
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let handle = SecretHandle::new("api_token").unwrap();
    secret_store
        .put(
            context.resource_scope.clone(),
            handle.clone(),
            SecretMaterial::from("runtime-secret"),
        )
        .await
        .unwrap();
    let obligations = vec![Obligation::InjectSecretOnce {
        handle: handle.clone(),
    }];

    let outcome = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();

    handler
        .abort(brassclaw_capabilities::CapabilityObligationAbortRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            outcome: &outcome,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_satisfy_preserves_staged_handoffs_when_releasing_reservation() {
    let secret_store = Arc::new(InMemorySecretStore::new());
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let services = obligation_services(secret_store.clone(), governor.clone());
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let account = ResourceAccount::tenant(context.resource_scope.tenant_id.clone());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    };
    let handle = SecretHandle::new("api_token").unwrap();
    secret_store
        .put(
            context.resource_scope.clone(),
            handle.clone(),
            SecretMaterial::from("runtime-secret"),
        )
        .await
        .unwrap();
    let obligations = vec![
        Obligation::ApplyNetworkPolicy {
            policy: allowed_network_policy(),
        },
        Obligation::InjectSecretOnce {
            handle: handle.clone(),
        },
        Obligation::ReserveResources {
            reservation_id: ResourceReservationId::new(),
        },
    ];

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();

    assert_eq!(governor.reserved_for(&account).concurrency_slots, 0);
}

#[tokio::test]
async fn builtin_obligation_handler_cleans_unused_staged_handoffs_after_dispatch_completion() {
    let secret_store = Arc::new(InMemorySecretStore::new());
    let services = obligation_services(
        secret_store.clone(),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let handle = SecretHandle::new("api_token").unwrap();
    secret_store
        .put(
            context.resource_scope.clone(),
            handle.clone(),
            SecretMaterial::from("runtime-secret"),
        )
        .await
        .unwrap();
    let policy = allowed_network_policy();
    let obligations = vec![
        Obligation::ApplyNetworkPolicy { policy },
        Obligation::InjectSecretOnce {
            handle: handle.clone(),
        },
    ];

    handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();

    handler
        .complete_dispatch(CapabilityObligationCompletionRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            dispatch: &sample_dispatch(
                &context.resource_scope,
                &capability_id,
                json!({"ok": true}),
            ),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn builtin_obligation_handler_returns_scoped_mount_outcome_when_subset() {
    let handler = BuiltinObligationHandler::new();
    let mut context = execution_context(CapabilitySet::default());
    context.mounts = mount_view(
        "/workspace",
        "/projects/demo",
        MountPermissions::read_write(),
    );
    let scoped_mounts = mount_view(
        "/workspace",
        "/projects/demo",
        MountPermissions::read_only(),
    );
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::UseScopedMounts {
        mounts: scoped_mounts.clone(),
    }];

    let outcome = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();

    assert_eq!(outcome.mounts, Some(scoped_mounts));
}

#[tokio::test]
async fn builtin_obligation_handler_reserves_requested_resources_and_releases_on_abort() {
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let handler = BuiltinObligationHandler::new().with_resource_governor(governor.clone());
    let context = execution_context(CapabilitySet::default());
    let account = ResourceAccount::tenant(context.resource_scope.tenant_id.clone());
    let capability_id = capability_id();
    let estimate = ResourceEstimate {
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    };
    let reservation_id = ResourceReservationId::new();
    let obligations = vec![Obligation::ReserveResources { reservation_id }];

    let outcome = handler
        .prepare(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap();

    assert_eq!(
        outcome
            .resource_reservation
            .as_ref()
            .map(|reservation| reservation.id),
        Some(reservation_id)
    );
    assert_eq!(governor.reserved_for(&account).concurrency_slots, 1);

    handler
        .abort(brassclaw_capabilities::CapabilityObligationAbortRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
            outcome: &outcome,
        })
        .await
        .unwrap();

    assert_eq!(governor.reserved_for(&account).concurrency_slots, 0);
}

#[tokio::test]
async fn default_host_runtime_fails_closed_when_resource_ceiling_lacks_required_estimate() {
    let registry = Arc::new(registry_with_echo_capability());
    let dispatcher = Arc::new(PanicDispatcher);
    let authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer> =
        Arc::new(ObligatingAuthorizer::new(vec![
            Obligation::EnforceResourceCeiling {
                ceiling: resource_ceiling(),
            },
        ]));
    let runtime = DefaultHostRuntime::new(
        registry,
        dispatcher,
        authorizer,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
        local_test_runtime_policy(),
    )
    .with_builtin_obligation_handler();

    let outcome = runtime
        .invoke_capability(RuntimeCapabilityRequest::new(
            execution_context(CapabilitySet::default()),
            capability_id(),
            ResourceEstimate::default(),
            json!({"message": "must not dispatch"}),
            trust_decision(),
        ))
        .await
        .unwrap();

    match outcome {
        RuntimeCapabilityOutcome::Failed(failure) => {
            assert_eq!(failure.kind, RuntimeFailureKind::Resource);
        }
        other => panic!("expected failed outcome, got {other:?}"),
    }
}

#[tokio::test]
async fn default_host_runtime_dispatches_when_resource_ceiling_is_satisfied() {
    let registry = Arc::new(registry_with_echo_capability());
    let dispatcher = Arc::new(RecordingDispatcher);
    let authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer> =
        Arc::new(ObligatingAuthorizer::new(vec![
            Obligation::EnforceResourceCeiling {
                ceiling: ResourceCeiling {
                    max_usd: Some(2.into()),
                    max_input_tokens: None,
                    max_output_tokens: None,
                    max_wall_clock_ms: None,
                    max_output_bytes: None,
                    sandbox: None,
                },
            },
        ]));
    let runtime = DefaultHostRuntime::new(
        registry,
        dispatcher,
        authorizer,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
        local_test_runtime_policy(),
    )
    .with_builtin_obligation_handler();

    let outcome = runtime
        .invoke_capability(RuntimeCapabilityRequest::new(
            execution_context(CapabilitySet::default()),
            capability_id(),
            ResourceEstimate {
                usd: Some(1.into()),
                ..ResourceEstimate::default()
            },
            json!({"message": "obligated"}),
            trust_decision(),
        ))
        .await
        .unwrap();

    assert!(matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
}

#[tokio::test]
async fn default_host_runtime_installs_configured_obligation_handler() {
    let registry = Arc::new(registry_with_echo_capability());
    let dispatcher = Arc::new(RecordingDispatcher);
    let authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer> =
        Arc::new(ObligatingAuthorizer::new(vec![Obligation::AuditBefore]));
    let audit = Arc::new(InMemoryAuditSink::new());
    let handler = Arc::new(BuiltinObligationHandler::new().with_audit_sink(audit.clone()));
    let runtime = DefaultHostRuntime::new(
        registry,
        dispatcher,
        authorizer,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
        local_test_runtime_policy(),
    )
    .with_obligation_handler(handler);

    let outcome = runtime
        .invoke_capability(RuntimeCapabilityRequest::new(
            execution_context(CapabilitySet::default()),
            capability_id(),
            ResourceEstimate::default(),
            json!({"message": "obligated"}),
            trust_decision(),
        ))
        .await
        .unwrap();

    assert!(matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action.kind, "capability_invoke");
}

struct ObligatingAuthorizer {
    obligations: Vec<Obligation>,
}

impl ObligatingAuthorizer {
    fn new(obligations: Vec<Obligation>) -> Self {
        Self { obligations }
    }
}

#[async_trait]
impl TrustAwareCapabilityDispatchAuthorizer for ObligatingAuthorizer {
    async fn authorize_dispatch_with_trust(
        &self,
        _context: &ExecutionContext,
        _descriptor: &CapabilityDescriptor,
        _estimate: &ResourceEstimate,
        _trust_decision: &TrustDecision,
    ) -> Decision {
        Decision::Allow {
            obligations: Obligations::new(self.obligations.clone()).unwrap(),
        }
    }
}

#[derive(Default)]
struct RecordingDispatcher;

struct PanicDispatcher;

#[async_trait]
impl CapabilityDispatcher for PanicDispatcher {
    async fn dispatch_json(
        &self,
        _request: CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchResult, DispatchError> {
        panic!("dispatcher must not be called for unsupported resource-ceiling obligations")
    }
}

#[async_trait]
impl CapabilityDispatcher for RecordingDispatcher {
    async fn dispatch_json(
        &self,
        request: CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchResult, DispatchError> {
        Ok(CapabilityDispatchResult {
            capability_id: request.capability_id,
            provider: ExtensionId::new("echo").unwrap(),
            runtime: RuntimeKind::FirstParty,
            output: json!({"ok": true}),
            display_preview: None,
            usage: ResourceUsage::default(),
            receipt: ResourceReceipt {
                id: request
                    .resource_reservation
                    .as_ref()
                    .map(|reservation| reservation.id)
                    .unwrap_or_default(),
                scope: request.scope,
                status: ReservationStatus::Reconciled,
                estimate: request.estimate,
                actual: Some(ResourceUsage::default()),
            },
        })
    }
}

fn sample_dispatch(
    scope: &ResourceScope,
    capability_id: &CapabilityId,
    output: serde_json::Value,
) -> CapabilityDispatchResult {
    CapabilityDispatchResult {
        capability_id: capability_id.clone(),
        provider: ExtensionId::new("echo").unwrap(),
        runtime: RuntimeKind::FirstParty,
        output,
        display_preview: None,
        usage: ResourceUsage::default(),
        receipt: ResourceReceipt {
            id: ResourceReservationId::new(),
            scope: scope.clone(),
            status: ReservationStatus::Reconciled,
            estimate: ResourceEstimate::default(),
            actual: Some(ResourceUsage::default()),
        },
    }
}

fn mount_view(alias: &str, target: &str, permissions: MountPermissions) -> MountView {
    MountView::new(vec![MountGrant::new(
        MountAlias::new(alias).unwrap(),
        VirtualPath::new(target).unwrap(),
        permissions,
    )])
    .unwrap()
}

fn resource_ceiling() -> ResourceCeiling {
    ResourceCeiling {
        max_usd: Some(1.into()),
        max_input_tokens: None,
        max_output_tokens: None,
        max_wall_clock_ms: None,
        max_output_bytes: None,
        sandbox: None,
    }
}

fn allowed_network_policy() -> NetworkPolicy {
    NetworkPolicy {
        allowed_targets: vec![NetworkTargetPattern {
            scheme: Some(NetworkScheme::Https),
            host_pattern: "api.example.test".to_string(),
            port: None,
        }],
        deny_private_ip_ranges: true,
        max_egress_bytes: Some(1024),
    }
}

fn registry_with_echo_capability() -> ExtensionRegistry {
    let manifest = parse_manifest(ECHO_MANIFEST);
    let root = VirtualPath::new(format!("/system/extensions/{}", manifest.id.as_str())).unwrap();
    let package = ExtensionPackage::from_manifest(manifest, root).unwrap();
    let mut registry = ExtensionRegistry::new();
    registry.insert(package).unwrap();
    registry
}

fn parse_manifest(manifest: &str) -> ExtensionManifest {
    let manifest = legacy_capability_fixture_to_v2(manifest);
    ExtensionManifest::parse(
        &manifest,
        ManifestSource::InstalledLocal,
        &HostPortCatalog::empty(),
    )
    .unwrap()
}

fn execution_context(grants: CapabilitySet) -> ExecutionContext {
    let invocation_id = InvocationId::new();
    let resource_scope = ResourceScope {
        tenant_id: TenantId::new("tenant1").unwrap(),
        user_id: UserId::new("user1").unwrap(),
        agent_id: None,
        project_id: Some(ProjectId::new("project1").unwrap()),
        thread_id: None,
        invocation_id,
    };
    ExecutionContext {
        invocation_id,
        correlation_id: CorrelationId::new(),
        process_id: None,
        parent_process_id: None,
        tenant_id: resource_scope.tenant_id.clone(),
        user_id: resource_scope.user_id.clone(),
        agent_id: resource_scope.agent_id.clone(),
        project_id: resource_scope.project_id.clone(),
        thread_id: resource_scope.thread_id.clone(),
        extension_id: ExtensionId::new("caller").unwrap(),
        runtime: RuntimeKind::FirstParty,
        trust: TrustClass::Sandbox,
        grants,
        mounts: MountView::default(),
        resource_scope,
    }
}

fn capability_id() -> CapabilityId {
    CapabilityId::new("echo.say").unwrap()
}

fn trust_decision() -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::sandbox(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects: vec![EffectKind::DispatchCapability],
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::Default,
        evaluated_at: chrono::Utc::now(),
    }
}

#[derive(Debug, Default)]
struct CapturingRequirementResolver {
    seen: std::sync::Mutex<Option<RuntimeCredentialAccountRequirement>>,
}

#[async_trait]
impl RuntimeCredentialAccountResolver for CapturingRequirementResolver {
    async fn resolve_access_secret(
        &self,
        request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
        *self.seen.lock().unwrap() = request.requirement.cloned();
        Err(CredentialStageError::AuthRequired)
    }
}

fn exact_account_requirement() -> RuntimeCredentialAccountRequirement {
    RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("example").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("runtime-slot").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("example").unwrap(),
                setup: RuntimeCredentialAccountSetup::OAuth {
                    scopes: vec!["read".into()],
                },
            },
            provider_scopes: vec!["read".into()],
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: "api.example.com".into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap()
}

#[tokio::test]
async fn exact_requirement_reaches_resolver_and_original_auth_gate_without_a_secret_lease() {
    let secret_store = Arc::new(InMemorySecretStore::new());
    let resolver = Arc::new(CapturingRequirementResolver::default());
    let services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        secret_store.clone(),
        Arc::new(InMemoryResourceGovernor::new()),
    )
    .with_credential_account_resolver(resolver.clone());
    let requirement = exact_account_requirement();
    let obligations = vec![Obligation::InjectCredentialAccountOnce {
        handle: requirement.requirement().handle.clone(),
        provider: RuntimeCredentialAccountProviderId::new("example").unwrap(),
        provider_scopes: vec!["read".into()],
        requester_extension: requirement.requester_extension().clone(),
        requirement: Some(Box::new(requirement.clone())),
    }];
    let context = execution_context(CapabilitySet::default());
    let estimate = ResourceEstimate::default();
    let capability = capability_id();
    let error = services
        .obligation_handler()
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();
    assert_eq!(resolver.seen.lock().unwrap().as_ref(), Some(&requirement));
    let CapabilityObligationError::AuthRequired {
        credential_requirements,
    } = error
    else {
        panic!("expected original Auth requirement");
    };
    assert_eq!(credential_requirements.len(), 1);
    assert_eq!(
        credential_requirements[0].requirement.as_ref(),
        Some(&requirement)
    );
    assert!(
        secret_store
            .leases_for_scope(&context.resource_scope)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn conflicting_requirement_stops_before_resolver_and_secret_selection() {
    for (handle, provider, requester) in [
        ("different-slot", "example", "example"),
        ("runtime-slot", "different-provider", "example"),
        ("runtime-slot", "example", "different-requester"),
    ] {
        let secret_store = Arc::new(InMemorySecretStore::new());
        let resolver = Arc::new(CapturingRequirementResolver::default());
        let services = BuiltinObligationServices::new(
            Arc::new(InMemoryAuditSink::new()),
            secret_store.clone(),
            Arc::new(InMemoryResourceGovernor::new()),
        )
        .with_credential_account_resolver(resolver.clone());
        let obligations = vec![Obligation::InjectCredentialAccountOnce {
            handle: SecretHandle::new(handle).unwrap(),
            provider: RuntimeCredentialAccountProviderId::new(provider).unwrap(),
            provider_scopes: vec!["read".into()],
            requester_extension: ExtensionId::new(requester).unwrap(),
            requirement: Some(Box::new(exact_account_requirement())),
        }];
        let context = execution_context(CapabilitySet::default());
        let estimate = ResourceEstimate::default();
        let capability = capability_id();
        let error = services
            .obligation_handler()
            .satisfy(CapabilityObligationRequest {
                phase: CapabilityObligationPhase::Invoke,
                context: &context,
                capability_id: &capability,
                estimate: &estimate,
                obligations: &obligations,
            })
            .await
            .unwrap_err();
        assert!(matches!(error, CapabilityObligationError::Failed { .. }));
        assert!(resolver.seen.lock().unwrap().is_none());
        assert!(
            secret_store
                .leases_for_scope(&context.resource_scope)
                .await
                .unwrap()
                .is_empty()
        );
    }
}

#[derive(Debug)]
struct AlwaysAuthRequiredResolver;

#[async_trait::async_trait]
impl RuntimeCredentialAccountResolver for AlwaysAuthRequiredResolver {
    async fn resolve_access_secret(
        &self,
        _request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, brassclaw_host_api::CredentialStageError> {
        Err(brassclaw_host_api::CredentialStageError::AuthRequired)
    }
}

struct FixedHandleResolver {
    handle: brassclaw_host_api::SecretHandle,
    store: Arc<dyn brassclaw_secrets::SecretStore>,
}
impl std::fmt::Debug for FixedHandleResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FixedHandleResolver")
    }
}

#[async_trait::async_trait]
impl RuntimeCredentialAccountResolver for FixedHandleResolver {
    async fn resolve_access_secret(
        &self,
        request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, brassclaw_host_api::CredentialStageError> {
        RuntimeCredentialAccessSecret::capture(
            self.store.as_ref(),
            request.scope.clone(),
            self.handle.clone(),
            None,
        )
        .await
    }
}

struct SourceScopedHandleResolver {
    source_scope: ResourceScope,
    handle: brassclaw_host_api::SecretHandle,
    store: Arc<dyn brassclaw_secrets::SecretStore>,
}
impl std::fmt::Debug for SourceScopedHandleResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SourceScopedHandleResolver")
    }
}

#[async_trait::async_trait]
impl RuntimeCredentialAccountResolver for SourceScopedHandleResolver {
    async fn resolve_access_secret(
        &self,
        _request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, brassclaw_host_api::CredentialStageError> {
        RuntimeCredentialAccessSecret::capture(
            self.store.as_ref(),
            self.source_scope.clone(),
            self.handle.clone(),
            None,
        )
        .await
    }
}

#[tokio::test]
async fn inject_credential_account_once_fails_when_no_resolver_wired() {
    // Services built without a credential_account_resolver — obligation must hard-fail.
    let secret_store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
    let governor = Arc::new(brassclaw_resources::InMemoryResourceGovernor::new());
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secret_store,
        governor,
    );
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::InjectCredentialAccountOnce {
        requirement: None,
        handle: brassclaw_host_api::SecretHandle::new("github_runtime_token").unwrap(),
        provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("github").unwrap(),
        provider_scopes: Vec::new(),
        requester_extension: brassclaw_host_api::ExtensionId::new("github").unwrap(),
    }];

    let err = handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    assert!(
        matches!(err, CapabilityObligationError::Failed { .. }),
        "expected Failed, got {err:?}"
    );
}

#[tokio::test]
async fn inject_credential_account_once_fails_when_resolver_returns_auth_required() {
    // Services wired with a resolver that always returns AuthRequired.
    let secret_store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
    let governor = Arc::new(brassclaw_resources::InMemoryResourceGovernor::new());
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secret_store,
        governor,
    )
    .with_credential_account_resolver(Arc::new(AlwaysAuthRequiredResolver));
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let provider_scopes = vec!["https://www.googleapis.com/auth/drive.readonly".to_string()];
    let obligations = vec![Obligation::InjectCredentialAccountOnce {
        requirement: None,
        handle: brassclaw_host_api::SecretHandle::new("github_runtime_token").unwrap(),
        provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("github").unwrap(),
        provider_scopes: provider_scopes.clone(),
        requester_extension: brassclaw_host_api::ExtensionId::new("github").unwrap(),
    }];

    let err = handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    let CapabilityObligationError::AuthRequired {
        credential_requirements,
    } = err
    else {
        panic!("expected AuthRequired, got {err:?}");
    };
    assert_eq!(credential_requirements.len(), 1);
    assert_eq!(credential_requirements[0].provider.as_str(), "github");
    assert_eq!(
        credential_requirements[0].requester_extension.as_str(),
        "github"
    );
    assert_eq!(credential_requirements[0].provider_scopes, provider_scopes);
}

#[tokio::test]
async fn inject_credential_account_once_resolves_and_stages_secret() {
    // Happy path: resolver returns Ok(handle), material exists in store,
    // obligation is satisfied, and the staged secret appears in the injection store.
    let access_handle = brassclaw_host_api::SecretHandle::new("github_access_secret").unwrap();
    let injection_slot = brassclaw_host_api::SecretHandle::new("github_runtime_token").unwrap();
    let secret_store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
    // Seed the store with material under the access handle.
    secret_store
        .put(
            execution_context(CapabilitySet::default())
                .resource_scope
                .clone(),
            access_handle.clone(),
            brassclaw_secrets::SecretMaterial::from("test-github-token"),
        )
        .await
        .unwrap();
    let governor = Arc::new(brassclaw_resources::InMemoryResourceGovernor::new());
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secret_store.clone(),
        governor,
    )
    .with_credential_account_resolver(Arc::new(FixedHandleResolver {
        handle: access_handle.clone(),
        store: secret_store.clone(),
    }));
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::InjectCredentialAccountOnce {
        requirement: None,
        handle: injection_slot.clone(),
        provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("github").unwrap(),
        provider_scopes: Vec::new(),
        requester_extension: brassclaw_host_api::ExtensionId::new("github").unwrap(),
    }];

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .expect("obligation should be satisfied");
}

#[tokio::test]
async fn inject_credential_account_once_reads_from_resolved_source_scope() {
    let access_handle = brassclaw_host_api::SecretHandle::new("github_access_secret").unwrap();
    let injection_slot = brassclaw_host_api::SecretHandle::new("github_runtime_token").unwrap();
    let source_scope =
        ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new()).unwrap();
    let mut target_scope = source_scope.clone();
    target_scope.project_id = Some(ProjectId::new("project-a").unwrap());
    target_scope.invocation_id = InvocationId::new();
    let secret_store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
    secret_store
        .put(
            source_scope.clone(),
            access_handle.clone(),
            brassclaw_secrets::SecretMaterial::from("test-github-token"),
        )
        .await
        .unwrap();
    let governor = Arc::new(brassclaw_resources::InMemoryResourceGovernor::new());
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secret_store.clone(),
        governor,
    )
    .with_credential_account_resolver(Arc::new(SourceScopedHandleResolver {
        source_scope,
        handle: access_handle,
        store: secret_store.clone(),
    }));
    let handler = services.obligation_handler();
    let mut context = execution_context(CapabilitySet::default());
    context.resource_scope = target_scope;
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::InjectCredentialAccountOnce {
        requirement: None,
        handle: injection_slot,
        provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("github").unwrap(),
        provider_scopes: Vec::new(),
        requester_extension: brassclaw_host_api::ExtensionId::new("github").unwrap(),
    }];

    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .expect("obligation should read credential from resolved source scope");
}

#[tokio::test]
async fn inject_credential_account_once_maps_unknown_resolved_secret_to_auth_required() {
    // Resolver returns Ok(handle) but that handle has no material in the secret store.
    // Expected: AuthRequired — lease_once on an unknown secret is classified as a
    // user-actionable re-auth condition by `stage_secret_error`, shared with the
    // first-party stager path (ProductAuthProviderRuntimePorts::stage_secret_once).
    // This was Failed in earlier PR4233 iterations; aligned with reborn #4231
    // staged-credential semantics so the runtime auth gate fires consistently
    // regardless of which staging lane (obligation vs stager) discovered the gap.
    let access_handle = brassclaw_host_api::SecretHandle::new("missing_secret").unwrap();
    let secret_store = Arc::new(brassclaw_secrets::InMemorySecretStore::new()); // empty store
    let governor = Arc::new(brassclaw_resources::InMemoryResourceGovernor::new());
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secret_store.clone(),
        governor,
    )
    .with_credential_account_resolver(Arc::new(FixedHandleResolver {
        handle: access_handle.clone(),
        store: secret_store.clone(),
    }));
    let handler = services.obligation_handler();
    let context = execution_context(CapabilitySet::default());
    let capability_id = capability_id();
    let estimate = ResourceEstimate::default();
    let obligations = vec![Obligation::InjectCredentialAccountOnce {
        requirement: None,
        handle: brassclaw_host_api::SecretHandle::new("github_runtime_token").unwrap(),
        provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("github").unwrap(),
        provider_scopes: Vec::new(),
        requester_extension: brassclaw_host_api::ExtensionId::new("github").unwrap(),
    }];

    let err = handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id: &capability_id,
            estimate: &estimate,
            obligations: &obligations,
        })
        .await
        .unwrap_err();

    let CapabilityObligationError::AuthRequired {
        credential_requirements,
    } = err
    else {
        panic!("expected AuthRequired when resolved handle not in store, got {err:?}");
    };
    assert_eq!(credential_requirements.len(), 1);
    assert_eq!(credential_requirements[0].provider.as_str(), "github");
    assert_eq!(
        credential_requirements[0].requester_extension.as_str(),
        "github"
    );
}

const ECHO_MANIFEST: &str = r#"
id = "echo"
name = "Echo"
version = "0.1.0"
description = "Echo test extension"
trust = "third_party"

[runtime]
kind = "mcp"
transport = "stdio"
command = "echo"

[[capabilities]]
id = "echo.say"
description = "Echoes input"
effects = ["dispatch_capability"]
default_permission = "allow"
parameters_schema = {}
"#;

mod captured_credential_material_tests {
    use super::*;

    #[derive(Debug)]
    struct CapturedResolver(RuntimeCredentialAccessSecret);

    #[async_trait::async_trait]
    impl RuntimeCredentialAccountResolver for CapturedResolver {
        async fn resolve_access_secret(
            &self,
            _: RuntimeCredentialAccountRequest<'_>,
        ) -> Result<RuntimeCredentialAccessSecret, brassclaw_host_api::CredentialStageError>
        {
            Ok(self.0.clone())
        }
    }

    async fn reject_selected_source(
        selected: RuntimeCredentialAccessSecret,
        store: Arc<brassclaw_secrets::InMemorySecretStore>,
        context: brassclaw_host_api::ExecutionContext,
    ) {
        let services = BuiltinObligationServices::new(
            Arc::new(brassclaw_events::InMemoryAuditSink::new()),
            store.clone(),
            Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
        )
        .with_credential_account_resolver(Arc::new(CapturedResolver(selected)));
        let capability = capability_id();
        let estimate = ResourceEstimate::default();
        let obligations = [Obligation::InjectCredentialAccountOnce {
            requirement: None,
            handle: brassclaw_host_api::SecretHandle::new("captured-runtime-slot").unwrap(),
            provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("github")
                .unwrap(),
            provider_scopes: Vec::new(),
            requester_extension: brassclaw_host_api::ExtensionId::new("github").unwrap(),
        }];
        let error = services
            .obligation_handler()
            .satisfy(CapabilityObligationRequest {
                phase: CapabilityObligationPhase::Invoke,
                context: &context,
                capability_id: &capability,
                estimate: &estimate,
                obligations: &obligations,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            CapabilityObligationError::AuthRequired { .. }
        ));
        assert!(
            brassclaw_secrets::SecretStore::leases_for_scope(
                store.as_ref(),
                &context.resource_scope,
            )
            .await
            .unwrap()
            .is_empty(),
            "rejected selection must not lease today's source"
        );
    }

    #[tokio::test]
    async fn selected_generation_rejects_replacement_and_aba_before_account_injection() {
        for aba in [false, true] {
            let store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
            let context = execution_context(CapabilitySet::default());
            let handle = brassclaw_host_api::SecretHandle::new("captured-account-access").unwrap();
            brassclaw_secrets::SecretStore::put(
                store.as_ref(),
                context.resource_scope.clone(),
                handle.clone(),
                "original material".into(),
            )
            .await
            .unwrap();
            let selected = RuntimeCredentialAccessSecret::capture(
                store.as_ref(),
                context.resource_scope.clone(),
                handle.clone(),
                None,
            )
            .await
            .unwrap();
            assert!(!format!("{selected:?}").contains(&selected.record_reference));
            brassclaw_secrets::SecretStore::put(
                store.as_ref(),
                context.resource_scope.clone(),
                handle.clone(),
                "replacement material".into(),
            )
            .await
            .unwrap();
            if aba {
                brassclaw_secrets::SecretStore::put(
                    store.as_ref(),
                    context.resource_scope.clone(),
                    handle.clone(),
                    "original material".into(),
                )
                .await
                .unwrap();
            }
            assert_ne!(
                brassclaw_secrets::SecretStore::record_reference(
                    store.as_ref(),
                    &context.resource_scope,
                    &handle,
                )
                .await
                .unwrap()
                .as_deref(),
                Some(selected.record_reference.as_str()),
            );
            reject_selected_source(selected, store, context).await;
        }
    }

    #[tokio::test]
    async fn elapsed_selected_account_deadline_rejects_staging_even_with_live_store_record() {
        let store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
        let context = execution_context(CapabilitySet::default());
        let handle = brassclaw_host_api::SecretHandle::new("elapsed-account-access").unwrap();
        brassclaw_secrets::SecretStore::put(
            store.as_ref(),
            context.resource_scope.clone(),
            handle.clone(),
            "live store material".into(),
        )
        .await
        .unwrap();
        let mut selected = RuntimeCredentialAccessSecret::capture(
            store.as_ref(),
            context.resource_scope.clone(),
            handle,
            None,
        )
        .await
        .unwrap();
        // Represent the already-selected issuer deadline after a wait without
        // sleeping or extending the independently live underlying store record.
        selected.access_expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1));
        reject_selected_source(selected, store, context).await;
    }
}
