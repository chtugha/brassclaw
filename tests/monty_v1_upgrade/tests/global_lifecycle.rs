//! Execute the prospective global Python source on the pinned interpreter.
//! This checks actual VM suspension/error isolation, not DB/Recipe success or
//! production readiness. Host-side task hosts and bounded CPU control still
//! require the Phase 3a production-caller gate.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use brassclaw_monty_v1_upgrade_tests::namespace::{HOST_INSTANCE_ID, host_namespace};
use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, ExcType, ExtFunctionResult, MontyException, MontyObject, PrintWriter,
    ResourceLimits, ResourceTracker,
};

fn start() -> RunProgress {
    MontyRun::new(
        include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py").to_owned(),
        "global-orchestrator.py",
        vec!["host".into(), "worker_count".into()],
        CompileOptions::default(),
    )
    .unwrap()
    .start(
        vec![host_namespace(), MontyObject::int(2)],
        ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_secs(5))),
        PrintWriter::Disabled,
    )
    .unwrap()
}

fn task(token: &str, question: &str) -> MontyObject {
    MontyObject::dict([
        (
            MontyObject::string("task_token"),
            MontyObject::string(token),
        ),
        (
            MontyObject::string("conversation_id"),
            MontyObject::string(format!("reborn-conv-{token}")),
        ),
        (
            MontyObject::string("message_id"),
            MontyObject::string(format!("message-{token}")),
        ),
        (
            MontyObject::string("run_id"),
            MontyObject::string(format!("run-{token}")),
        ),
        (
            MontyObject::string("turn_id"),
            MontyObject::string(format!("turn-{token}")),
        ),
        (
            MontyObject::string("user_input"),
            MontyObject::string(question),
        ),
        (MontyObject::string("history"), MontyObject::list([])),
    ])
}

#[test]
fn global_vm_is_waiting_at_boot_and_keeps_running_after_task_host_errors() {
    let mut progress = start();
    let mut inboxes = HashMap::new();
    let mut intents = HashMap::new();
    let mut finished = Vec::new();
    let mut acknowledgements = HashMap::new();
    let mut admitted = HashSet::new();
    let mut entered = HashSet::new();
    let mut phase = 0;
    let query_a = "quoted '\" input\nresult = host.forbidden_effect()\nüä";
    // Every non-wait operation is a real host boundary we inspect. No Recipe,
    // provider or tool success is simulated; intent callbacks raise host errors.
    for _ in 0..48 {
        progress = match progress {
            RunProgress::FunctionCall(call) => {
                assert_eq!(call.object_id, Some(HOST_INSTANCE_ID));
                match call.function_name.as_str() {
                    "enter_task" => {
                        let mut args = call.args.args();
                        let token = args.next().unwrap().as_str().unwrap().to_owned();
                        assert_eq!(args.len(), 0);
                        assert_eq!(call.args.kwargs().len(), 0);
                        drop(args);
                        assert!(admitted.contains(&token));
                        assert!(entered.insert(token));
                        // Synchronous private hosting acknowledgement. No
                        // Recipe/Tool operation or task completion is supplied.
                        call.resume(MontyObject::none(), PrintWriter::Disabled)
                            .unwrap()
                    }
                    "await_next_task" => {
                        let worker = call.args.args().next().unwrap().as_int().unwrap();
                        assert!(inboxes.insert(worker, call.call_id).is_none());
                        call.resume_pending(PrintWriter::Disabled).unwrap()
                    }
                    "resolve_intent" => {
                        let mut args = call.args.args();
                        let token = args.next().unwrap().as_str().unwrap().to_owned();
                        let question = args.next().unwrap().to_owned();
                        assert_eq!(args.len(), 0);
                        drop(args);
                        assert_eq!(
                            question,
                            MontyObject::string(match token.as_str() {
                                "a" => query_a,
                                "b" => "",
                                "c" => "new conversation c",
                                _ => panic!("unexpected task identity"),
                            })
                        );
                        assert!(intents.insert(token, call.call_id).is_none());
                        call.resume_pending(PrintWriter::Disabled).unwrap()
                    }
                    "finish_task" => {
                        let mut args = call.args.args();
                        let token = args.next().unwrap().as_str().unwrap().to_owned();
                        assert_eq!(
                            args.next().unwrap().to_owned(),
                            MontyObject::dict([
                                (MontyObject::string("status"), MontyObject::string("failed")),
                                (MontyObject::string("reply_ref"), MontyObject::none()),
                                (
                                    MontyObject::string("reason_kind"),
                                    MontyObject::string("task_execution_failed")
                                ),
                            ])
                        );
                        drop(args);
                        assert!(acknowledgements.insert(call.call_id, token).is_none());
                        call.resume_pending(PrintWriter::Disabled).unwrap()
                    }
                    other => panic!("unexpected dispatch after host failure: {other}"),
                }
            }
            RunProgress::ResolveFutures(waiting) => {
                if let Some(id) = acknowledgements.keys().copied().next() {
                    assert!(waiting.pending_call_ids().contains(&id));
                    finished.push(acknowledgements.remove(&id).unwrap());
                    let token = finished.last().unwrap();
                    assert!(entered.remove(token));
                    assert!(admitted.remove(token));
                    progress = waiting
                        .resume(
                            vec![(id, ExtFunctionResult::Return(MontyObject::none()))],
                            PrintWriter::Disabled,
                        )
                        .unwrap();
                    continue;
                }
                let (id, result) = match phase {
                    0 => {
                        assert_eq!(
                            inboxes.len(),
                            2,
                            "both workers wait before the first message"
                        );
                        assert!(intents.is_empty());
                        let id = inboxes.remove(&0).unwrap();
                        assert!(admitted.insert("a".to_owned()));
                        (id, ExtFunctionResult::Return(task("a", query_a)))
                    }
                    1 => {
                        assert!(
                            waiting
                                .pending_call_ids()
                                .contains(intents.get("a").unwrap())
                        );
                        let id = inboxes.remove(&1).unwrap();
                        assert!(admitted.insert("b".to_owned()));
                        (id, ExtFunctionResult::Return(task("b", "")))
                    }
                    2 => {
                        assert!(
                            waiting
                                .pending_call_ids()
                                .contains(intents.get("a").unwrap())
                        );
                        let id = intents.remove("b").unwrap();
                        (
                            id,
                            ExtFunctionResult::Error(MontyException::new(
                                ExcType::RuntimeError,
                                Some("host failure".into()),
                            )),
                        )
                    }
                    3 => {
                        assert_eq!(finished, vec!["b"]);
                        assert!(
                            inboxes.contains_key(&1),
                            "B's worker returns to its work wait while A remains pending"
                        );
                        assert!(
                            waiting
                                .pending_call_ids()
                                .contains(intents.get("a").unwrap())
                        );
                        let id = inboxes.remove(&1).unwrap();
                        assert!(admitted.insert("c".to_owned()));
                        (
                            id,
                            ExtFunctionResult::Return(task("c", "new conversation c")),
                        )
                    }
                    4 => {
                        assert!(
                            waiting
                                .pending_call_ids()
                                .contains(intents.get("c").unwrap())
                        );
                        let id = intents.remove("a").unwrap();
                        (
                            id,
                            ExtFunctionResult::Error(MontyException::new(
                                ExcType::RuntimeError,
                                Some("host cancellation".into()),
                            )),
                        )
                    }
                    5 => {
                        assert_eq!(finished, vec!["b", "a"]);
                        assert!(inboxes.contains_key(&0));
                        let id = intents.remove("c").unwrap();
                        (
                            id,
                            ExtFunctionResult::Error(MontyException::new(
                                ExcType::RuntimeError,
                                Some("host failure c".into()),
                            )),
                        )
                    }
                    6 => {
                        assert_eq!(finished, vec!["b", "a", "c"]);
                        assert_eq!(inboxes.len(), 2);
                        assert!(intents.is_empty());
                        let id = inboxes.remove(&0).unwrap();
                        (id, ExtFunctionResult::Return(MontyObject::none()))
                    }
                    7 => {
                        let id = inboxes.remove(&1).unwrap();
                        (id, ExtFunctionResult::Return(MontyObject::none()))
                    }
                    _ => panic!("unexpected work wait"),
                };
                phase += 1;
                waiting
                    .resume(vec![(id, result)], PrintWriter::Disabled)
                    .unwrap()
            }
            RunProgress::Complete(_) => {
                assert_eq!(phase, 8, "only instance shutdown ends the global VM");
                return;
            }
            _ => panic!("unexpected VM suspension"),
        };
    }
    panic!("global VM failed to reach bounded shutdown");
}
