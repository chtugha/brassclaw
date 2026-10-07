//! Typed data boundary for the global Monty service's admitted task host.
//!
//! One call invokes one existing host operation. This adapter does not select
//! Recipes, iterate the model, execute tool batches or complete a run. The
//! owning service resolves the private routing token to an exact admitted host
//! before entering here; conversation IDs and claim tokens are not parameters.

use serde::{
    Serialize,
    de::{DeserializeOwned, IntoDeserializer},
};
use serde_json::Value;

use brassclaw_turns::{
    LoopMessageRef,
    run_profile::{
        AgentLoopHostError, AgentLoopHostErrorKind, AppendCapabilityResultRef, AssistantReply,
        CapabilityInvocation, FinalizeAssistantMessage, LoopModelRequest, LoopPromptBundleRequest,
    },
};

use crate::monty_task_host::MontyTaskHost;

impl MontyTaskHost {
    /// Invoke one task-scoped host port using data exported from Monty.
    /// Unknown fields fail before the host is called. In particular, permissive
    /// serde DTOs must not silently consume an attempted authority override.
    pub async fn dispatch_port(
        &self,
        name: &str,
        payload: Value,
    ) -> Result<Value, AgentLoopHostError> {
        match name {
            "visible_capabilities" => {
                fields(&payload, &[])?;
                encode(self.visible_capabilities().await?)
            }
            "build_prompt_bundle" => {
                let request: LoopPromptBundleRequest = decode(
                    payload,
                    &[
                        "mode",
                        "context_cursor",
                        "surface_version",
                        "capability_view",
                        "checkpoint_state_ref",
                        "max_messages",
                        "inline_messages",
                    ],
                )?;
                encode(self.build_prompt_bundle(request).await?)
            }
            "stream_model" => {
                let request: LoopModelRequest = decode(
                    payload,
                    &[
                        "messages",
                        "surface_version",
                        "model_preference",
                        "capability_view",
                    ],
                )?;
                encode(self.stream_model(request).await?)
            }
            "invoke_capability" => {
                let request: CapabilityInvocation =
                    decode(payload, &["surface_version", "capability_id", "input_ref"])?;
                encode(self.invoke_capability(request).await?)
            }
            "append_capability_result_ref" => {
                let request: AppendCapabilityResultRef = decode(
                    payload,
                    &[
                        "result_ref",
                        "safe_summary",
                        "provider_call",
                        "model_observation",
                    ],
                )?;
                encode(self.append_capability_result_ref(request).await?)
            }
            "post_reply" => {
                fields(&payload, &["answer"])?;
                let answer = payload
                    .get("answer")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_payload)?;
                let reference = self
                    .finalize_assistant_message(FinalizeAssistantMessage {
                        reply: AssistantReply {
                            content: answer.to_owned(),
                        },
                    })
                    .await?;
                encode(reference)
            }
            "resolve_reply" => {
                fields(&payload, &["reply_ref"])?;
                let reference: LoopMessageRef = serde_json::from_value(
                    payload
                        .get("reply_ref")
                        .cloned()
                        .ok_or_else(invalid_payload)?,
                )
                .map_err(|_| invalid_payload())?;
                encode(self.published_reply_content(&reference)?)
            }
            _ => Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "Monty task port is not registered",
            )),
        }
    }
}

fn fields(payload: &Value, allowed: &[&str]) -> Result<(), AgentLoopHostError> {
    let object = payload.as_object().ok_or_else(invalid_payload)?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid_payload());
    }
    Ok(())
}

fn decode<T: DeserializeOwned>(payload: Value, allowed: &[&str]) -> Result<T, AgentLoopHostError> {
    fields(&payload, allowed)?;
    let mut unknown_field = false;
    let result = serde_ignored::deserialize(payload.into_deserializer(), |_| unknown_field = true)
        .map_err(|_| invalid_payload())?;
    if unknown_field {
        return Err(invalid_payload());
    }
    Ok(result)
}

fn encode<T: Serialize>(result: T) -> Result<Value, AgentLoopHostError> {
    serde_json::to_value(result).map_err(|_| {
        AgentLoopHostError::new(
            AgentLoopHostErrorKind::Unavailable,
            "Monty task port result encoding failed",
        )
    })
}

fn invalid_payload() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::InvalidInvocation,
        "Monty task port requires its declared typed inputs",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_port_rejects_nested_authority_fields_and_raw_prompt_bypasses() {
        for payload in [
            json!({"messages": [{"role": "user", "content_ref": "msg:input", "lease_token": "injected"}]}),
            json!({"messages": [], "capability_view": {"visible_capability_ids": [], "grant": true}}),
            json!({"messages": [], "resolved_messages": null}),
            json!({"messages": [], "conversation_id": "another-chat"}),
        ] {
            let result: Result<LoopModelRequest, _> = decode(
                payload,
                &[
                    "messages",
                    "surface_version",
                    "model_preference",
                    "capability_view",
                ],
            );
            assert_eq!(
                result.unwrap_err().kind,
                AgentLoopHostErrorKind::InvalidInvocation
            );
        }
        let request: LoopModelRequest = decode(
            json!({
                "messages": [{"role": "user", "content_ref": "msg:input"}],
            }),
            &["messages"],
        )
        .unwrap();
        assert_eq!(request.messages[0].content_ref.as_str(), "msg:input");
        assert!(request.resolved_messages.is_none());
    }
}
