//! Native preflight evidence, never inferred from a provider error category.
use super::*;
use brassclaw_llm::final_request_guard::{
    FinalRequestError, FinalRequestGuard, FinalRequestInput, FinalRequestMeasurement,
    FinalRequestMeasurer,
};
use brassclaw_turns::run_profile::{LoopModelResponse, LoopModelResponseCustodian};

struct Custody;
#[async_trait]
impl LoopModelResponseCustodian for Custody {
    fn requires_provider_observation(&self) -> bool {
        true
    }
    fn receive_response(&self, _response: &LoopModelResponse) -> Result<(), AgentLoopHostError> {
        panic!("preflight rejection received an answer")
    }
    async fn persist_response(&self) -> Result<(), AgentLoopHostError> {
        panic!("preflight rejection persisted an answer")
    }
}
#[derive(Debug)]
struct UnusedPolicy;
#[async_trait]
impl brassclaw_llm::ProviderDispatchGuard for UnusedPolicy {
    async fn check(
        &self,
        _facts: brassclaw_llm::ProviderRequestFacts<'_>,
    ) -> Result<brassclaw_llm::ProviderDispatchState, LlmError> {
        panic!("unsupported adapter entered measurement policy")
    }
}
struct UnusedMeasurement;
#[async_trait]
impl FinalRequestMeasurer for UnusedMeasurement {
    async fn measure(
        &self,
        _input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError> {
        panic!("unsupported adapter entered measurement")
    }
}
struct Unsupported;
#[async_trait]
impl LlmProvider for Unsupported {
    fn model_name(&self) -> std::borrow::Cow<'_, str> {
        "model".into()
    }
    fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
        (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
    }
    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        panic!("unsupported adapter dispatched")
    }
    async fn complete_with_tools(
        &self,
        _request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        panic!("unsupported adapter dispatched")
    }
}

#[tokio::test]
async fn error_category_alone_is_not_native_submission_evidence() {
    let custody: Option<Arc<dyn LoopModelResponseCustodian>> = Some(Arc::new(Custody));
    let identity = ProviderReplayIdentity::new("provider", "model").unwrap();
    let fabricated = observed_provider_call::<()>(&custody, &identity, async {
        Err(LlmError::FinalRequestRejected {
            reason: FinalRequestError::UnsupportedTransport,
        })
    })
    .await
    .unwrap_err();
    assert_eq!(
        fabricated.kind,
        HostManagedModelErrorKind::ConfigurationError
    );
    assert!(
        !fabricated.confirmed_unsubmitted(),
        "an error label cannot release unresolved work"
    );
    let guard = Arc::new(
        FinalRequestGuard::new(
            "provider".into(),
            "model".into(),
            "http://127.0.0.1:1/v1/chat/completions".parse().unwrap(),
            Arc::new(UnusedPolicy),
            Arc::new(UnusedMeasurement),
        )
        .unwrap(),
    );
    let native = observed_provider_call(
        &custody,
        &identity,
        guard.complete(
            &Unsupported,
            CompletionRequest::new(vec![]).with_model("model"),
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(native.kind, HostManagedModelErrorKind::ConfigurationError);
    assert!(
        native.confirmed_unsubmitted(),
        "the actual native preflight barrier records no submission"
    );
}

struct BoundCustody(Option<brassclaw_turns::run_profile::LoopModelInputReference>);
#[async_trait]
impl LoopModelResponseCustodian for BoundCustody {
    fn retained_model_input_reference(
        &self,
    ) -> Option<&brassclaw_turns::run_profile::LoopModelInputReference> {
        self.0.as_ref()
    }
    fn requires_provider_observation(&self) -> bool {
        true
    }
    fn receive_response(&self, _: &LoopModelResponse) -> Result<(), AgentLoopHostError> {
        panic!("input preparation cannot receive provider responses")
    }
    async fn persist_response(&self) -> Result<(), AgentLoopHostError> {
        panic!("input preparation cannot persist provider responses")
    }
}
fn input_guard() -> Arc<FinalRequestGuard> {
    Arc::new(
        FinalRequestGuard::new(
            "provider".into(),
            "model".into(),
            "http://127.0.0.1:1/v1/chat/completions".parse().unwrap(),
            Arc::new(UnusedPolicy),
            Arc::new(UnusedMeasurement),
        )
        .unwrap(),
    )
}
fn input_binding(
    selection: brassclaw_turns::run_profile::RecipePrefixSelection,
) -> brassclaw_turns::run_profile::LoopModelInputReference {
    brassclaw_turns::run_profile::LoopModelInputReference {
        selection,
        reference: "original-retained-input".into(),
    }
}
#[test]
fn final_prefix_free_input_preserves_fixed_instructions_guard_and_output_reserve() {
    use brassclaw_turns::run_profile::RecipePrefixSelection;
    let binding = input_binding(RecipePrefixSelection::PrefixFreeWorkflow);
    let owner = BoundCustody(Some(binding.clone()));
    let guard = input_guard();
    let mut completion = CompletionRequest::new(vec![
        ChatMessage::system("exact fixed workflow instructions"),
        ChatMessage::user("exact data"),
    ]);
    let before = serde_json::to_value(&completion.messages).unwrap();
    let result = TaskFinalModelRequest {
        guard: guard.clone(),
        binding,
        input: TaskFinalModelInput::PrefixFreeWorkflow,
        request_output_tokens: std::num::NonZeroU32::new(17),
    }
    .apply(&mut completion, Some(&owner))
    .unwrap();
    assert!(
        Arc::ptr_eq(&guard, &result),
        "omission must retain the actual final guard"
    );
    assert_eq!(serde_json::to_value(&completion.messages).unwrap(), before);
    assert_eq!(completion.max_tokens, Some(17));
}
#[test]
fn final_input_rejects_changed_identity_or_selection_before_mutating_request() {
    use brassclaw_turns::run_profile::RecipePrefixSelection::{
        Default, PrefixFreeWorkflow, Selected,
    };
    for (selection, expected, free) in [
        (Default, Some(input_binding(Selected)), false),
        (Selected, Some(input_binding(Default)), false),
        (PrefixFreeWorkflow, Some(input_binding(Default)), true),
        (Default, Some(input_binding(PrefixFreeWorkflow)), false),
        (PrefixFreeWorkflow, None, true),
        (Default, None, false),
        (Default, Some(input_binding(Default)), true),
        (
            PrefixFreeWorkflow,
            Some(input_binding(PrefixFreeWorkflow)),
            false,
        ),
    ] {
        let owner = BoundCustody(expected);
        let mut completion = CompletionRequest::new(vec![ChatMessage::user("original")]);
        let before = serde_json::to_value(&completion.messages).unwrap();
        let result = TaskFinalModelRequest {
            guard: input_guard(),
            binding: input_binding(selection),
            input: if free {
                TaskFinalModelInput::PrefixFreeWorkflow
            } else {
                TaskFinalModelInput::Prefix {
                    reference: "exact prefix".into(),
                }
            },
            request_output_tokens: std::num::NonZeroU32::new(17),
        }
        .apply(&mut completion, Some(&owner));
        assert!(result.unwrap_err().confirmed_unsubmitted());
        assert_eq!(serde_json::to_value(&completion.messages).unwrap(), before);
        assert_eq!(completion.max_tokens, None);
    }
    let mut changed = input_binding(Default);
    changed.reference = "different retained generation".into();
    let owner = BoundCustody(Some(changed));
    let mut completion = CompletionRequest::new(vec![]);
    assert!(
        TaskFinalModelRequest {
            guard: input_guard(),
            binding: input_binding(Default),
            input: TaskFinalModelInput::Prefix {
                reference: "exact prefix".into()
            },
            request_output_tokens: std::num::NonZeroU32::new(17),
        }
        .apply(&mut completion, Some(&owner))
        .is_err()
    );
    assert!(completion.messages.is_empty());
}
#[test]
fn final_present_prefix_keeps_explicit_output_limit_and_inserts_once() {
    use brassclaw_turns::run_profile::RecipePrefixSelection::Selected;
    let binding = input_binding(Selected);
    let owner = BoundCustody(Some(binding.clone()));
    let mut completion = CompletionRequest::new(vec![
        ChatMessage::system("dynamic system"),
        ChatMessage::user("data"),
    ]);
    completion.max_tokens = Some(23);
    TaskFinalModelRequest {
        guard: input_guard(),
        binding,
        input: TaskFinalModelInput::Prefix {
            reference: "selected exact prefix".into(),
        },
        request_output_tokens: std::num::NonZeroU32::new(17),
    }
    .apply(&mut completion, Some(&owner))
    .unwrap();
    assert_eq!(completion.messages.len(), 3);
    assert_eq!(
        serde_json::to_value(&completion.messages[0]).unwrap()["content"],
        "selected exact prefix"
    );
    assert_eq!(
        serde_json::to_value(&completion.messages[1]).unwrap()["content"],
        "dynamic system"
    );
    assert_eq!(completion.max_tokens, Some(23));
}

#[test]
fn final_input_needs_an_actual_positive_output_reservation_before_mutating_messages() {
    use brassclaw_turns::run_profile::RecipePrefixSelection::{PrefixFreeWorkflow, Selected};
    for selection in [PrefixFreeWorkflow, Selected] {
        for (explicit, fallback, expected) in [
            (None, None, None),
            (Some(0), Some(17), None),
            (Some(23), None, Some(23)),
            (Some(23), Some(17), Some(23)),
            (None, Some(29), Some(29)),
        ] {
            let binding = input_binding(selection);
            let owner = BoundCustody(Some(binding.clone()));
            let guard = input_guard();
            let mut completion = CompletionRequest::new(vec![ChatMessage::user("original")]);
            completion.max_tokens = explicit;
            let before = serde_json::to_value(&completion.messages).unwrap();
            let outcome = TaskFinalModelRequest {
                guard: guard.clone(),
                binding,
                input: if selection == PrefixFreeWorkflow {
                    TaskFinalModelInput::PrefixFreeWorkflow
                } else {
                    TaskFinalModelInput::Prefix {
                        reference: "exact prefix".into(),
                    }
                },
                request_output_tokens: fallback.and_then(std::num::NonZeroU32::new),
            }
            .apply(&mut completion, Some(&owner));
            if let Some(expected) = expected {
                let actual_guard = outcome.unwrap();
                assert!(Arc::ptr_eq(&guard, &actual_guard));
                assert_eq!(completion.max_tokens, Some(expected));
                assert_eq!(
                    completion.messages.len(),
                    if selection == PrefixFreeWorkflow {
                        1
                    } else {
                        2
                    }
                );
            } else {
                assert!(outcome.unwrap_err().confirmed_unsubmitted());
                assert_eq!(serde_json::to_value(&completion.messages).unwrap(), before);
                assert_eq!(completion.max_tokens, explicit);
            }
        }
    }
}
