//! One-operation host primitives. Python owns sequencing; these adapters never
//! replay the original turn, select a next step or activate a candidate.
use crate::global_monty_owner::GlobalOwnerCheck;
use async_trait::async_trait;
use brassclaw_host_api::{ResourceUsage, RuntimeDispatchErrorKind, ThreadId};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityHandler, FirstPartyCapabilityRequest,
    FirstPartyCapabilityResult,
};
use brassclaw_loop_support::{
    HostManagedModelGateway, HostManagedModelMessage, HostManagedModelMessageRole,
    HostManagedModelRequest,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    completed_turn_analysis,
    component_revision::REVISION_LIMITS,
    review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
    value_contract::strict_json,
};
use brassclaw_threads::{SessionThreadService, ThreadHistoryRequest, ThreadScope};
use brassclaw_turns::{
    LoopMessageRef,
    run_profile::{
        LoopModelBudgetAccountant, LoopModelResponse, LoopRunContext, ModelCallOutcome,
        ModelProfileId, ModelStreamChunk, ModelWorkKind, ModelWorkRequest, ParentLoopOutput,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

pub(super) fn error() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::OperationFailed)
}
pub(super) fn checksum(bytes: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes.as_bytes()))
}
pub(super) fn parse(bytes: &str) -> Result<Value, FirstPartyCapabilityError> {
    strict_json(bytes, REVISION_LIMITS).map_err(|_| error())
}
pub(super) struct PinnedModel {
    pub name: String,
    pub gateway: Arc<dyn HostManagedModelGateway>,
    pub prefix: String,
    pub accountant: Arc<dyn LoopModelBudgetAccountant>,
}
pub(super) struct Backend {
    pub pool: Arc<PgPool>,
    pub source: Uuid,
    pub attempt: Uuid,
    pub owner: Uuid,
    pub ownership: GlobalOwnerCheck,
    pub closed: AtomicBool,
    pub observed_model: tokio::sync::Mutex<Option<String>>,
    pub threads: Arc<dyn SessionThreadService>,
    pub scope: ThreadScope,
    pub context: LoopRunContext,
    pub model: PinnedModel,
    pub selection: String,
    pub instructions: String,
}
impl Backend {
    pub(super) async fn check(&self) -> Result<(), FirstPartyCapabilityError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(error());
        }
        self.ownership.check().await.map_err(|_| error())?;
        let row = self.pool.get().await.map_err(|_|error())?.query_opt(
            "SELECT w.owner_id,w.selection_bytes,w.prefix_bytes,w.model_identity,w.event_checksum=e.event_checksum FROM brassclaw_monty_review_work w JOIN brassclaw_monty_review_events e ON e.run_id=w.source_run_id WHERE w.source_run_id=$1 AND w.attempt_id=$2",
            &[&self.source,&self.attempt]).await.map_err(|_|error())?.ok_or_else(error)?;
        if row.get::<_, Uuid>(0) != self.owner
            || row.get::<_, String>(1) != self.selection
            || row.get::<_, String>(2) != self.model.prefix
            || row.get::<_, String>(3) != self.model.name
            || !row.get::<_, bool>(4)
        {
            return Err(error());
        }
        Ok(())
    }
    async fn previous(
        &self,
        operation: &str,
        bytes: &str,
    ) -> Result<(), FirstPartyCapabilityError> {
        let row = self.pool.get().await.map_err(|_|error())?.query_opt(
            "SELECT output_bytes FROM brassclaw_monty_review_operations WHERE attempt_id=$1 AND operation=$2",
            &[&self.attempt,&operation]).await.map_err(|_|error())?.ok_or_else(error)?;
        let answer: String = row.get::<_, Option<String>>(0).ok_or_else(error)?;
        let answer = parse(&answer)?;
        if answer["kind"] != "return" || answer["value"].as_str() != Some(bytes) {
            return Err(error());
        }
        Ok(())
    }
    async fn transition(&self, from: &str, to: &str) -> Result<(), FirstPartyCapabilityError> {
        let changed = self.pool.get().await.map_err(|_|error())?.execute(
            "UPDATE brassclaw_monty_review_work SET phase=$3 WHERE attempt_id=$1 AND owner_id=$2 AND phase=$4",
            &[&self.attempt,&self.owner,&to,&from]).await.map_err(|_|error())?;
        if changed != 1 {
            return Err(error());
        }
        Ok(())
    }
    pub(super) async fn run(
        &self,
        operation: &str,
        bytes: &str,
    ) -> Result<String, FirstPartyCapabilityError> {
        self.check().await?;
        match operation {
            "read_event" => {
                if bytes != self.source.to_string() {
                    return Err(error());
                }
                let row = self.pool.get().await.map_err(|_|error())?.query_one(
                    "SELECT event_bytes,event_checksum FROM brassclaw_monty_review_events WHERE run_id=$1",
                    &[&self.source]).await.map_err(|_|error())?;
                let event: String = row.get(0);
                let digest: String = row.get(1);
                if checksum(&event) != digest {
                    return Err(error());
                }
                Ok(json!({"event_bytes":event,"event_checksum":digest}).to_string())
            }
            "qualify_evidence" => {
                self.previous("read_event", bytes).await?;
                self.qualify(bytes).await
            }
            "claim_review" => {
                self.previous("qualify_evidence", bytes).await?;
                let bundle = parse(bytes)?;
                if bundle["evidence_complete"] != true {
                    return Err(error());
                }
                self.transition("qualified", "claimed").await?;
                Ok(
                    json!({"format":"completed-turn-review-claim/1","attempt_ref":self.attempt,
                    "bundle_bytes":bytes,"bundle_checksum":checksum(bytes),"evidence_complete":true,
                    "prefix_bytes":self.model.prefix,"prefix_checksum":checksum(&self.model.prefix),
                    "selection_bytes":self.selection})
                    .to_string(),
                )
            }
            "ask_sempai" => self.ask(bytes).await,
            "decode_proposals" => {
                self.previous("ask_sempai", bytes).await?;
                let packet = parse(bytes)?;
                let original = packet["response_bytes"].as_str().ok_or_else(error)?;
                let decoded =
                    completed_turn_analysis::decode(self.attempt, original).map_err(|_| error())?;
                Ok(json!({"attempt_ref":self.attempt,"response_bytes":decoded.original_response_bytes,
                    "response_checksum":hex::encode(decoded.response_checksum),
                    "candidate_count":decoded.candidates.len()}).to_string())
            }
            "submit_candidates" => {
                self.previous("decode_proposals", bytes).await?;
                let record = parse(bytes)?;
                let decoded = completed_turn_analysis::decode(
                    self.attempt,
                    record["response_bytes"].as_str().ok_or_else(error)?,
                )
                .map_err(|_| error())?;
                let mut receipts = Vec::new();
                // One explicit batch-retention operation, with stable IDs and
                // individually immutable subjects. Partial receipts survive a
                // failure; no model redispatch or whole-Recipe retry is allowed.
                for candidate in decoded.candidates {
                    self.check().await?;
                    let draft = ReviewSubmissionDraft::new(
                        candidate.submission_id,
                        "completed-turn-sempai",
                        candidate.candidate,
                        candidate.base,
                        candidate.dependencies,
                    )
                    .map_err(|_| error())?;
                    let stored = PgReviewSubmissionStore::new(self.pool.clone())
                        .submit(&draft)
                        .await
                        .map_err(|_| error())?;
                    receipts.push(json!({"submission_id":stored.id,"subject_checksum":stored.checksum,
                        "candidate":{"uuid":stored.candidate.uuid,"version":stored.candidate.version,
                            "class_code":stored.candidate.class_code,"checksum":hex::encode(stored.candidate.checksum)},
                        "review_status":"unreviewed","catalogue_activated":false}));
                }
                self.transition("model_returned", "submitted").await?;
                Ok(json!({"attempt_ref":self.attempt,"response_checksum":record["response_checksum"],
                    "submissions":receipts,"catalogue_activated":false}).to_string())
            }
            "acknowledge_review" => {
                self.previous("submit_candidates", bytes).await?;
                let receipt =
                    json!({"format":"completed-turn-review-receipt/1","attempt_id":self.attempt,
                    "source_run_id":self.source,"status":"submitted_unreviewed",
                    "submission_receipts":parse(bytes)?,"catalogue_activated":false})
                    .to_string();
                let changed = self.pool.get().await.map_err(|_|error())?.execute(
                    "UPDATE brassclaw_monty_review_work SET phase='acknowledged',receipt_bytes=$3
                     WHERE attempt_id=$1 AND owner_id=$2 AND phase='submitted'",
                    &[&self.attempt,&self.owner,&receipt]).await.map_err(|_|error())?;
                if changed != 1 {
                    return Err(error());
                }
                Ok(format!("review:{}", self.attempt))
            }
            _ => Err(error()),
        }
    }

    async fn qualify(&self, bytes: &str) -> Result<String, FirstPartyCapabilityError> {
        let read = parse(bytes)?;
        let event_bytes = read["event_bytes"].as_str().ok_or_else(error)?;
        let event = parse(event_bytes)?;
        if event["run_id"] != self.source.to_string()
            || event["scope"]["tenant_id"] != self.scope.tenant_id.to_string()
            || event["outcome"]["execution"]["intent_outcome"] != "no_match"
        {
            return Err(error());
        }
        let thread = ThreadId::new(event["scope"]["thread_id"].as_str().ok_or_else(error)?)
            .map_err(|_| error())?;
        let history = self
            .threads
            .list_thread_history(ThreadHistoryRequest {
                scope: self.scope.clone(),
                thread_id: thread,
            })
            .await
            .map_err(|_| error())?;
        let accepted = event["accepted_message_ref"].as_str().ok_or_else(error)?;
        let reply = event["outcome"]["reply_ref"]
            .as_str()
            .or_else(|| event["outcome"]["execution"]["reply"]["reference"].as_str());
        let start = history
            .messages
            .iter()
            .find(|m| format!("msg:{}", m.message_id) == accepted);
        let end = reply.and_then(|reference| {
            history
                .messages
                .iter()
                .find(|m| format!("msg:{}", m.message_id) == reference)
        });
        let mut diagnostics = Vec::<&str>::new();
        use brassclaw_threads::{MessageKind, MessageStatus};
        if start.is_some_and(|m| m.kind != MessageKind::User || m.content.is_none())
            || end.is_some_and(|m| {
                m.kind != MessageKind::Assistant || m.status != MessageStatus::Finalized
            })
            || start
                .zip(end)
                .is_some_and(|(s, e)| e.sequence <= s.sequence)
        {
            diagnostics.push("transcript_boundary_invalid");
        }
        if event["outcome"]["execution"]["withheld_host_answers"] == true
            || event["outcome"]["execution"]["withheld_root_answers"]
                .as_u64()
                .is_none_or(|n| n != 0)
        {
            diagnostics.push("withheld_effects_unresolved");
        }
        if start.is_none_or(|m| m.turn_run_id.as_deref() != Some(&self.source.to_string())) {
            diagnostics.push("accepted_message_missing_or_mismatched");
        }
        if end.is_none_or(|m| {
            m.turn_run_id.as_deref() != Some(&self.source.to_string()) || m.content.is_none()
        }) {
            diagnostics.push("finalized_reply_boundary_missing");
        }
        let bound = end.map(|m| m.sequence).unwrap_or(0);
        let transcript: Vec<_> = history
            .messages
            .iter()
            .filter(|m| m.sequence <= bound)
            .map(|m| json!({"message":m,"provider_tool_call":m.tool_result_provider_call}))
            .collect();
        if transcript
            .iter()
            .any(|m| m["message"]["redaction_ref"].is_string())
        {
            diagnostics.push("redacted_history");
        }
        let packets = event["model_packets"].as_array().ok_or_else(error)?;
        let expected = event["outcome"]["execution"]["model_exchange_count"].as_u64();
        if expected.is_none_or(|n| n == 0 || n as usize != packets.len()) {
            diagnostics.push("model_capture_count_incomplete");
        }
        let mut seen = BTreeSet::new();
        let mut calls = Vec::<Value>::new();
        let mut final_content = None;
        for (index, packet) in packets.iter().enumerate() {
            let exchange = &packet["model_exchange"];
            if packet["iteration"].as_u64() != Some(index as u64)
                || !seen.insert(packet["id"].clone().to_string())
                || exchange["format"] != "host-model-exchange/1"
                || !exchange["original_request_bytes"].is_string()
                || !exchange["effective_request_bytes"].is_string()
                || !exchange["response_bytes"].is_string()
                || !exchange["failure"].is_null()
            {
                diagnostics.push("model_exchange_unresolved");
                continue;
            }
            let response: brassclaw_loop_support::HostManagedModelResponse =
                serde_json::from_value(parse(
                    exchange["response_bytes"].as_str().ok_or_else(error)?,
                )?)
                .map_err(|_| error())?;
            match response.output {
                ParentLoopOutput::AssistantReply(reply) => {
                    final_content = Some(reply.content);
                }
                ParentLoopOutput::CapabilityCalls(items) => {
                    if items.is_empty() {
                        diagnostics.push("empty_capability_dispatch");
                    }
                    for call in items {
                        calls.push(serde_json::to_value(call).map_err(|_| error())?);
                    }
                    final_content = None;
                }
            }
            for field in ["original_request_bytes", "effective_request_bytes"] {
                let request = parse(exchange[field].as_str().ok_or_else(error)?)?;
                if request["run_id"] != event["run_id"] || request["turn_id"] != event["turn_id"] {
                    diagnostics.push("model_exchange_identity_mismatch");
                }
            }
        }
        if final_content.as_deref() != end.and_then(|m| m.content.as_deref())
            || final_content.is_none()
        {
            diagnostics.push("terminal_model_reply_mismatch");
        }
        let client = self.pool.get().await.map_err(|_| error())?;
        let rows = client
            .query(
                "SELECT reference,kind,invocation_id,capability_id,payload
            FROM brassclaw_loop_capability_values WHERE run_id=$1 AND scope=$2 ORDER BY reference",
                &[&self.source, &event["scope"]],
            )
            .await
            .map_err(|_| error())?;
        let results = rows
            .iter()
            .filter(|r| r.get::<_, String>(1) == "result")
            .count();
        if event["outcome"]["execution"]["capability_dispatch_count"].as_u64()
            != Some(results as u64)
        {
            diagnostics.push("capability_results_incomplete");
        }
        let values: Vec<_> = rows
            .iter()
            .map(|r| {
                json!({"reference":r.get::<_,String>(0),
            "kind":r.get::<_,String>(1),"invocation_id":r.get::<_,Option<Uuid>>(2),
            "capability_id":r.get::<_,Option<String>>(3),"payload":r.get::<_,Value>(4)})
            })
            .collect();
        let mut used_results = BTreeSet::new();
        for call in calls {
            let input_ref = call["input_ref"].as_str().ok_or_else(error)?;
            let input = values
                .iter()
                .find(|v| v["kind"] == "input" && v["reference"] == input_ref);
            let replay = &call["provider_replay"];
            let message = history.messages.iter().find(|m| {
                m.sequence <= bound
                    && m.turn_run_id.as_deref() == Some(&self.source.to_string())
                    && m.tool_result_provider_call.as_ref().is_some_and(|r| {
                        r.provider_call_id == replay["provider_call_id"].as_str().unwrap_or("")
                            && r.provider_turn_id
                                == replay["provider_turn_id"].as_str().unwrap_or("")
                            && r.capability_id.as_str()
                                == call["capability_id"].as_str().unwrap_or("")
                            && input.is_some_and(|v| v["payload"] == r.arguments)
                    })
            });
            let result = message
                .and_then(|m| m.tool_result_ref.as_deref())
                .and_then(|reference| {
                    values.iter().find(|v| {
                        v["kind"] == "result"
                            && v["reference"] == reference
                            && v["capability_id"] == call["capability_id"]
                    })
                });
            if result.is_none_or(|v| !used_results.insert(v["reference"].to_string())) {
                diagnostics.push("model_tool_values_missing");
            }
        }
        if used_results.len() != results {
            diagnostics.push("unpaired_capability_result");
        }
        for invocation in event["tool_invocations"].as_array().ok_or_else(error)? {
            for (field, hash) in [
                ("arguments_bytes", "arguments_checksum"),
                ("answer_bytes", "answer_checksum"),
            ] {
                if invocation[field]
                    .as_str()
                    .is_none_or(|bytes| invocation[hash] != checksum(bytes))
                {
                    diagnostics.push("recipe_effect_checksum_invalid");
                }
            }
        }
        if event["tool_invocations"]
            .as_array()
            .ok_or_else(error)?
            .iter()
            .any(|i| i["phase"] != "answered" || !i["answer_bytes"].is_string())
        {
            diagnostics.push("recipe_effects_unresolved");
        }
        let complete = diagnostics.is_empty();
        let bundle = json!({"format":"completed-turn-review-bundle/1","event_bytes":event_bytes,
            "event_checksum":read["event_checksum"],"transcript":transcript,
            "capability_values":values,"evidence_complete":complete,"diagnostics":diagnostics})
        .to_string();
        parse(&bundle)?; // capacity rejects the whole bundle, never truncates
        let receipt = (!complete).then(||json!({"format":"completed-turn-review-receipt/1",
            "attempt_id":self.attempt,"source_run_id":self.source,"status":"incomplete_evidence",
            "bundle_checksum":checksum(&bundle),"diagnostics":diagnostics,"catalogue_activated":false}).to_string());
        let phase = if complete { "qualified" } else { "incomplete" };
        let changed = client
            .execute(
                "UPDATE brassclaw_monty_review_work SET phase=$3,bundle_bytes=$4,receipt_bytes=$5
            WHERE attempt_id=$1 AND owner_id=$2 AND phase='reserved'",
                &[&self.attempt, &self.owner, &phase, &bundle, &receipt],
            )
            .await
            .map_err(|_| error())?;
        if changed != 1 || !complete {
            return Err(error());
        }
        Ok(bundle)
    }

    async fn ask(&self, bytes: &str) -> Result<String, FirstPartyCapabilityError> {
        let value = parse(bytes)?;
        let request: AnalysisRequest = serde_json::from_value(value).map_err(|_| error())?;
        let row=self.pool.get().await.map_err(|_|error())?.query_one(
            "SELECT bundle_bytes,phase FROM brassclaw_monty_review_work WHERE attempt_id=$1 AND owner_id=$2",
            &[&self.attempt,&self.owner]).await.map_err(|_|error())?;
        let bundle: String = row.get::<_, Option<String>>(0).ok_or_else(error)?;
        if row.get::<_, String>(1) != "claimed"
            || request.format != "completed-turn-sempai-request/1"
            || request.attempt_ref != self.attempt.to_string()
            || request.bundle_checksum != checksum(&bundle)
            || request.prefix_checksum != checksum(&self.model.prefix)
            || request.selection_bytes != self.selection
            || !request.tools.is_empty()
            || request.messages.len() != 3
            || request.messages[0].role != "system"
            || request.messages[0].content != self.model.prefix
            || request.messages[1].role != "system"
            || request.messages[1].content != self.instructions
            || request.messages[2].role != "user"
            || request.messages[2].content != bundle
        {
            return Err(error());
        }
        let messages = request
            .messages
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                Ok(HostManagedModelMessage {
                    role: if m.role == "system" {
                        HostManagedModelMessageRole::System
                    } else {
                        HostManagedModelMessageRole::User
                    },
                    content: m.content,
                    content_ref: LoopMessageRef::new(format!(
                        "msg:review-input-{}-{i}",
                        self.attempt
                    ))
                    .map_err(|_| error())?,
                    tool_result_provider_call: None,
                    tool_result_content: None,
                })
            })
            .collect::<Result<Vec<_>, FirstPartyCapabilityError>>()?;
        let profile = ModelProfileId::new("sempai_model").map_err(|_| error())?;
        let work = ModelWorkRequest {
            kind: ModelWorkKind::SempaiReview,
            model_profile_id: profile.clone(),
            resolved_model_route: None,
            estimated_input_tokens: messages
                .iter()
                .map(|m| m.content.chars().count() as u64)
                .sum::<u64>()
                .div_ceil(4),
            estimated_output_tokens: None,
        };
        self.model
            .accountant
            .pre_model_work(&self.context, &work)
            .await
            .map_err(|_| error())?;
        let mut reservation = Reservation {
            accountant: self.model.accountant.clone(),
            context: self.context.clone(),
            armed: true,
        };
        let changed=self.pool.get().await.map_err(|_|error())?.execute(
            "UPDATE brassclaw_monty_review_work SET phase='model_dispatching',model_dispatch_count=1,request_bytes=$3
             WHERE attempt_id=$1 AND owner_id=$2 AND phase='claimed' AND model_dispatch_count=0",
            &[&self.attempt,&self.owner,&bytes]).await.map_err(|_|error())?;
        if changed != 1 {
            return Err(error());
        }
        self.check().await?;
        let response = self
            .model
            .gateway
            .stream_model(HostManagedModelRequest {
                model_profile_id: profile.clone(),
                messages,
                surface_version: None,
                resolved_model_route: None,
                run_id: self.context.run_id,
                turn_id: self.context.turn_id,
            })
            .await;
        // Once the gateway has been entered, unknown spend requires reconciliation.
        reservation.armed = false;
        let response = match response {
            Ok(response) => response,
            Err(failure) => {
                let observed=json!({"format":"completed-turn-provider-failure/1","kind":format!("{:?}",failure.kind),
                    "provider_outcome":"unresolved"}).to_string();
                *self.observed_model.lock().await = Some(observed.clone());
                let changed = self.pool.get().await.map_err(|_|error())?.execute(
                    "UPDATE brassclaw_monty_review_work SET phase='uncertain',response_bytes=$2 WHERE attempt_id=$1 AND owner_id=$3 AND phase='model_dispatching'",
                    &[&self.attempt,&observed,&self.owner]).await.map_err(|_|error())?;
                if changed != 1 {
                    return Err(error());
                }
                return Err(error());
            }
        };
        let exact = serde_json::to_string(&response).map_err(|_| {
            tracing::warn!(stage="serialize_model_response", "internal review response retention failed");
            error()
        })?;
        *self.observed_model.lock().await = Some(exact.clone());
        let changed = self.pool.get().await.map_err(|_|error())?.execute(
            "UPDATE brassclaw_monty_review_work SET phase='model_returned',response_bytes=$2 WHERE attempt_id=$1 AND owner_id=$3 AND phase='model_dispatching'",
            &[&self.attempt,&exact,&self.owner]).await.map_err(|failure| {
                tracing::warn!(stage="persist_model_response", sqlstate=failure.code().map(|c| c.code()),
                    "internal review response retention failed");
                error()
            })?;
        if changed != 1 {
            return Err(error());
        }
        let usage = LoopModelResponse {
            chunks: response
                .safe_text_deltas
                .into_iter()
                .map(|safe_text_delta| ModelStreamChunk { safe_text_delta })
                .collect(),
            safe_reasoning_deltas: response.safe_reasoning_deltas,
            output: response.output,
            effective_model_profile_id: profile,
            usage: response.usage,
        };
        let reconciled = self
            .model
            .accountant
            .post_model_work_result(&self.context, &work, ModelCallOutcome::Success(&usage))
            .await;
        reservation.armed = false; // failed reconcile retains actual reservation
        reconciled.map_err(|_| error())?;
        let ParentLoopOutput::AssistantReply(reply) = usage.output else {
            return Err(error());
        };
        Ok(json!({"attempt_ref":self.attempt,"response_bytes":reply.content,"provider_response_checksum":checksum(&exact)}).to_string())
    }
}
struct Reservation {
    accountant: Arc<dyn LoopModelBudgetAccountant>,
    context: LoopRunContext,
    armed: bool,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.armed {
            self.accountant.release_in_flight(&self.context);
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalysisRequest {
    format: String,
    attempt_ref: String,
    bundle_checksum: String,
    prefix_checksum: String,
    selection_bytes: String,
    messages: Vec<Message>,
    tools: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    role: String,
    content: String,
}

pub(crate) struct ReviewPrimitive {
    pub(super) backend: Arc<Backend>,
    pub(super) operation: &'static str,
}
#[async_trait]
impl FirstPartyCapabilityHandler for ReviewPrimitive {
    async fn dispatch(
        &self,
        request: FirstPartyCapabilityRequest,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        if request.scope.tenant_id != self.backend.context.scope.tenant_id
            || request.scope.thread_id.as_ref() != Some(&self.backend.context.thread_id)
            || request.capability_id.as_str() != format!("host.review_{}", self.operation)
            || request.input.as_object().is_none_or(|v| v.len() != 1)
        {
            return Err(error());
        }
        let bytes = request.input["record_bytes"].as_str().ok_or_else(error)?;
        let result = self.backend.run(self.operation, bytes).await?;
        Ok(FirstPartyCapabilityResult::new(
            json!(result),
            ResourceUsage::default(),
        ))
    }
}
