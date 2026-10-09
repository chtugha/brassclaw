//! Actual instance service, worker, file I/O and cancellation boundaries.
//! The fixture ports perform real reads/writes; they do not synthesize provider,
//! composition or Tool results and do not establish production kernel acceptance.
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use brassclaw_monty_host::{
    process::TaskHandle,
    service::{PortFailure, ServiceFailure, ServiceOwner, TaskInput, TaskOutcome, TaskPorts},
    transport_actor::{ActorLimits, StopKind, TransportClient},
};
use futures::{FutureExt, future::BoxFuture};
use serde_json::{Value, json};
use tokio::sync::Notify;

#[path = "support/runtime.rs"]
mod support;
use support::{boot, limits, worker};

const FILE_ROOT: &str = r#"
import asyncio
async def worker(worker_id):
    while True:
        task = await host.await_next_task(worker_id)
        if task is None:
            return
        token = task['task_token']
        outcome = {'status': 'failed', 'reply_ref': None, 'reason_kind': 'task_execution_failed'}
        try:
            host.enter_task(token)
            value = await host.read_file(token, {
                'conversation_id': task['conversation_id'], 'message_id': task['message_id'],
                'turn_id': task['turn_id'], 'run_id': task['run_id'],
                'user_input': task['user_input'], 'history': task['history']
            })
            reference = await host.post_reply(token, value)
            outcome = {'status': 'completed', 'reply_ref': reference, 'reason_kind': None}
        except Exception as error:
            if str(error) == 'task_cancelled':
                outcome['reason_kind'] = 'task_cancelled'
        await host.finish_task(token, outcome)
        task = None
        token = None
        value = None
        reference = None
        outcome = None
await asyncio.gather(*[worker(i) for i in range(worker_count)])
None
"#;

struct HeldChild {
    task: TaskHandle,
    parent: brassclaw_monty_host::process::RecipeContextId,
    transport: TransportClient,
}

struct FilePorts {
    input: PathBuf,
    reply: PathBuf,
    expected: Value,
    fenced: AtomicBool,
    reads: AtomicUsize,
    writes: AtomicUsize,
    started: Notify,
    release: Notify,
    pause_read: bool,
    panic_after_read: bool,
    retain_child_heap: bool,
    retained_vm_bytes: AtomicUsize,
    resume_child_after_edit: bool,
    child_resumed: AtomicBool,
    child_opened: Option<tokio::sync::mpsc::Sender<HeldChild>>,
}
impl FilePorts {
    fn new(root: &std::path::Path, label: &str, pause_read: bool) -> Arc<Self> {
        let input = root.join(format!("input-{label}"));
        std::fs::write(&input, format!("actual file content {label}: Ü {{values}}"))
            .expect("actual source file");
        Arc::new(Self {
            input,
            reply: root.join(format!("reply-{label}")),
            expected: json!({
                "conversation_id": format!("opaque conversation {label}"),
                "message_id": format!("message {label}"), "turn_id": format!("turn {label}"),
                "run_id": format!("run {label}"), "user_input": "'quoted' Ü {{values}}",
                "history": [{"role": "user", "content": "prior typed history"}]
            }),
            fenced: AtomicBool::new(false),
            reads: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            started: Notify::new(),
            release: Notify::new(),
            pause_read,
            panic_after_read: false,
            retain_child_heap: false,
            retained_vm_bytes: AtomicUsize::new(0),
            resume_child_after_edit: false,
            child_resumed: AtomicBool::new(false),
            child_opened: None,
        })
    }
    fn input(&self) -> TaskInput {
        TaskInput {
            conversation_id: self.expected["conversation_id"].as_str().unwrap().into(),
            message_id: self.expected["message_id"].as_str().unwrap().into(),
            turn_id: self.expected["turn_id"].as_str().unwrap().into(),
            run_id: self.expected["run_id"].as_str().unwrap().into(),
            user_input: self.expected["user_input"].as_str().unwrap().into(),
            history: self.expected["history"].as_array().unwrap().clone(),
        }
    }

    async fn run_retained_child(
        &self,
        transport: &TransportClient,
        task: TaskHandle,
        content: String,
    ) -> Result<String, PortFailure> {
        use brassclaw_monty_host::process::{
            RecipeBoundary, RecipeCommand, RecipeEvent, SelectedPython, WorkerCommand,
        };
        use sha2::{Digest, Sha256};

        let exchange = async |command| {
            transport
                .try_submit(WorkerCommand::Recipe { command })
                .map_err(|_| failure("child_transport_failed"))?
                .wait()
                .await
                .map_err(|_| failure("child_transport_failed"))?
                .outcome
                .map_err(|_| failure("child_transport_failed"))
        };
        let opened = exchange(RecipeCommand::Open { task, parent: None }).await?;
        let Some(RecipeEvent::Opened { context, .. }) = opened.recipe else {
            return Err(failure("child_transport_failed"));
        };
        if let Some(observer) = &self.child_opened {
            observer
                .send(HeldChild {
                    task,
                    parent: context,
                    transport: transport.clone(),
                })
                .await
                .map_err(|_| failure("child_transport_failed"))?;
        }
        // The returned file content is small, while the child keeps actual
        // interpreter storage until the service closes this completed task.
        let source = "held = 'x' * 2097152\nresult = inputs['content']";
        let mut progress = exchange(RecipeCommand::Start {
            context,
            selected: SelectedPython {
                source: source.into(),
                checksum: Sha256::digest(source.as_bytes()).into(),
                aliases: Default::default(),
            },
            inputs: json!({"content": content}),
        })
        .await?;
        loop {
            match progress.recipe {
                Some(RecipeEvent::Progress {
                    boundary: RecipeBoundary::ControlYield { key },
                    ..
                }) => {
                    progress = exchange(RecipeCommand::ResumeControl { context, key }).await?;
                }
                Some(RecipeEvent::Progress {
                    boundary:
                        RecipeBoundary::Complete {
                            value: Value::String(content),
                        },
                    ..
                }) => {
                    self.retained_vm_bytes
                        .store(progress.vm_live_bytes, Ordering::Release);
                    if self.resume_child_after_edit {
                        self.started.notify_one();
                        self.release.notified().await;
                        let followup =
                            "result = inputs['content'] if len(held) == 2097152 else 'lost state'";
                        let mut next = exchange(RecipeCommand::Start {
                            context,
                            selected: SelectedPython {
                                source: followup.into(),
                                checksum: Sha256::digest(followup.as_bytes()).into(),
                                aliases: Default::default(),
                            },
                            inputs: json!({"content": content}),
                        })
                        .await?;
                        loop {
                            match next.recipe {
                                Some(RecipeEvent::Progress {
                                    boundary: RecipeBoundary::ControlYield { key },
                                    ..
                                }) => {
                                    next = exchange(RecipeCommand::ResumeControl { context, key })
                                        .await?;
                                }
                                Some(RecipeEvent::Progress {
                                    boundary:
                                        RecipeBoundary::Complete {
                                            value: Value::String(value),
                                        },
                                    ..
                                }) => {
                                    self.child_resumed.store(true, Ordering::Release);
                                    // A third feed must fail: the live reduction must not
                                    // reset the first feed already consumed by this child.
                                    let exhausted = transport
                                        .try_submit(WorkerCommand::Recipe {
                                            command: RecipeCommand::Start {
                                                context,
                                                selected: SelectedPython {
                                                    source: followup.into(),
                                                    checksum: Sha256::digest(followup.as_bytes())
                                                        .into(),
                                                    aliases: Default::default(),
                                                },
                                                inputs: json!({"content": "must never execute"}),
                                            },
                                        })
                                        .unwrap()
                                        .wait()
                                        .await
                                        .unwrap();
                                    assert!(matches!(
                                        exhausted.outcome,
                                        Err(brassclaw_monty_host::process::ProcessError {
                                            kind: brassclaw_monty_host::process::ProcessFailure::Vm(
                                                brassclaw_monty_host::VmFailure::SourceLimit
                                            ),
                                            ..
                                        })
                                    ));
                                    return Ok(value);
                                }
                                _ => return Err(failure("child_execution_failed")),
                            }
                        }
                    }
                    return Ok(content);
                }
                _ => return Err(failure("child_execution_failed")),
            }
        }
    }
}
fn failure(reason: &str) -> PortFailure {
    PortFailure::new(reason).unwrap()
}
impl TaskPorts for FilePorts {
    fn call(
        self: Arc<Self>,
        task: TaskHandle,
        transport: TransportClient,
        name: String,
        args: Vec<Value>,
        kwargs: BTreeMap<String, Value>,
    ) -> BoxFuture<'static, Result<Value, PortFailure>> {
        async move {
            if self.fenced.load(Ordering::Acquire) {
                return Err(failure("task_cancelled"));
            }
            if args.len() != 1 || !kwargs.is_empty() {
                return Err(failure("invalid_file_input"));
            }
            match name.as_str() {
                "read_file" => {
                    if args[0] != self.expected {
                        return Err(failure("wrong_task_input"));
                    }
                    self.reads.fetch_add(1, Ordering::AcqRel);
                    let child_value = if self.retain_child_heap {
                        let value = tokio::fs::read_to_string(&self.input)
                            .await
                            .map_err(|_| failure("file_read_failed"))?;
                        Some(self.run_retained_child(&transport, task, value).await?)
                    } else {
                        None
                    };
                    self.started.notify_one();
                    if self.pause_read {
                        self.release.notified().await;
                    }
                    let value = match child_value {
                        Some(value) => value,
                        None => tokio::fs::read_to_string(&self.input)
                            .await
                            .map_err(|_| failure("file_read_failed"))?,
                    };
                    assert!(!self.panic_after_read, "actual port panic after file read");
                    Ok(Value::String(value))
                }
                "post_reply" => {
                    let value = args[0].as_str().ok_or_else(|| failure("invalid_reply"))?;
                    tokio::fs::write(&self.reply, value.as_bytes())
                        .await
                        .map_err(|_| failure("file_write_failed"))?;
                    self.writes.fetch_add(1, Ordering::AcqRel);
                    Ok(Value::String(format!(
                        "msg:{}",
                        self.expected["message_id"].as_str().unwrap()
                    )))
                }
                _ => Err(failure("unknown_file_port")),
            }
        }
        .boxed()
    }
    fn finish(
        self: Arc<Self>,
        outcome: TaskOutcome,
    ) -> BoxFuture<'static, Result<TaskOutcome, PortFailure>> {
        async move {
            if let TaskOutcome::Completed { reply_ref } = &outcome {
                if reply_ref != &format!("msg:{}", self.expected["message_id"].as_str().unwrap()) {
                    return Err(failure("wrong_reply_reference"));
                }
                let actual = tokio::fs::read_to_string(&self.reply)
                    .await
                    .map_err(|_| failure("reply_not_persisted"))?;
                let source = tokio::fs::read_to_string(&self.input)
                    .await
                    .map_err(|_| failure("file_read_failed"))?;
                if actual != source {
                    return Err(failure("wrong_reply_content"));
                }
            }
            Ok(outcome)
        }
        .boxed()
    }
    fn fence(&self) {
        self.fenced.store(true, Ordering::Release);
    }
}

async fn start(source: &str) -> ServiceOwner {
    start_with_capacity(source, 4).await
}
async fn start_with_capacity(source: &str, queue_capacity: usize) -> ServiceOwner {
    let mut boot = boot(source);
    boot.aliases
        .extend(["read_file".into(), "post_reply".into()]);
    ServiceOwner::start(
        worker(),
        boot,
        limits(),
        ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: limits().max_frame_bytes * 16,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: limits().max_frame_bytes * 16,
        },
        queue_capacity,
    )
    .await
    .unwrap()
}
async fn graceful(owner: &mut ServiceOwner) {
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        exit.failure,
        None,
        "failed exchange kinds: {:?}; lifecycle: {:?}; outstanding: {}",
        exit.failed_exchanges
            .iter()
            .map(|receipt| receipt.outcome.as_ref().err().map(|error| error.kind))
            .collect::<Vec<_>>(),
        exit.last_snapshot.lifecycle,
        exit.last_snapshot.outstanding.len()
    );
    assert!(exit.tasks.is_empty());
    assert!(exit.pending_admission.is_none());
    assert!(exit.queued_admissions.is_empty());
    assert!(exit.failed_exchanges.is_empty());
    assert!(exit.rejected_commands.is_empty());
    assert!(exit.transport_inbox.outstanding().unwrap().is_empty());
    let transport = exit.transport.unwrap();
    assert_eq!(transport.kind, StopKind::Graceful);
    assert!(transport.exit_status.unwrap().success());
    assert!(transport.containment_error.is_none());
    assert!(transport.reap_error.is_none());
}

#[tokio::test]
async fn idle_native_worker_death_closes_service_without_a_new_task_or_control_rpc() {
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let pid = owner.worker_process_id().unwrap();
    assert!(
        tokio::process::Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    let exit = tokio::time::timeout(Duration::from_secs(2), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.failure, Some(ServiceFailure::Transport));
    assert!(exit.tasks.is_empty());
    assert!(exit.pending_admission.is_none());
    assert!(exit.failed_exchanges.is_empty());
    assert!(exit.transport_inbox.outstanding().unwrap().is_empty());
    let transport = exit.transport.unwrap();
    assert_eq!(transport.kind, StopKind::TransportFailed);
    assert!(!transport.exit_status.unwrap().success());
    assert!(transport.containment_error.is_none());
    assert!(transport.reap_error.is_none());
    let directory = tempfile::tempdir().unwrap();
    let ports = FilePorts::new(directory.path(), "after-exit", false);
    assert!(matches!(
        client.submit(ports.input(), ports),
        Err(ServiceFailure::Closed)
    ));
}

#[tokio::test]
async fn worker_death_during_real_wait_fences_and_retains_late_io_before_service_settlement() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let ports = FilePorts::new(directory.path(), "dying", true);
    let ticket = owner.client().submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    let pid = owner.worker_process_id().unwrap();
    assert!(
        tokio::process::Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ports.fenced.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    assert_eq!(
        ticket.control().stopped(Duration::from_millis(20)).await,
        Err(ServiceFailure::Deadline)
    );
    ports.release.notify_one();
    let exit = tokio::time::timeout(Duration::from_secs(2), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.failure, Some(ServiceFailure::Transport));
    assert_eq!(exit.tasks.len(), 1);
    assert_eq!(
        exit.tasks[0].host_results,
        [json!(
            tokio::fs::read_to_string(&ports.input).await.unwrap()
        )]
    );
    assert!(exit.tasks[0].reported_outcome.is_none());
    assert!(!ports.reply.exists());
    assert!(matches!(
        ticket.wait().await,
        Err(ServiceFailure::Transport)
    ));
    assert_eq!(exit.transport.unwrap().kind, StopKind::TransportFailed);
}

#[tokio::test]
async fn live_adapter_reserve_preserves_heap_identity_and_waiting_task_accounting() {
    use brassclaw_monty_host::{process::TaskSettings, service::ServiceHostingLimits};
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let identity = client.root_identity();
    let heap = client.heap_observation().status;
    let ports = FilePorts::new(directory.path(), "reserve", true);
    let ticket = client.submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), ports.started.notified())
        .await
        .unwrap();
    let settings = |revision| TaskSettings {
        revision,
        max_compute_time: Duration::from_secs(600),
        token_budgets_enabled: false,
    };
    let hosting = |reserve| ServiceHostingLimits {
        adapter_reserve_bytes: Some(reserve),
        admission: client.admission_observation().limits,
        max_pending_settings: client.settings_capacity().limit,
        max_retained_attempts: client.retained_attempts().limit,
        actor: None,
        deadlines: None,
    };
    let grown = client
        .publish_control_settings(
            1,
            settings(2),
            client.vm_bounds(),
            8,
            hosting(8 * 1024 * 1024),
        )
        .await
        .unwrap();
    let reduced = client
        .publish_control_settings(2, settings(3), client.vm_bounds(), 8, hosting(0))
        .await
        .unwrap();
    assert_eq!(grown.allocator.adapter_reserve_bytes, 8 * 1024 * 1024);
    assert_eq!(reduced.allocator.adapter_reserve_bytes, 0);
    assert_eq!(
        reduced.allocator.non_vm_reserve_bytes,
        2 * limits().max_frame_bytes
    );
    assert_eq!(
        reduced.allocator.memory_budget_bytes,
        heap.effective.unwrap().max_vm_bytes + 2 * limits().max_frame_bytes
    );
    assert_eq!(client.heap_observation().status, heap);
    assert_eq!(client.root_identity(), identity);
    assert_eq!(grown.accounting.len(), 1);
    assert_eq!(reduced.accounting.len(), 1);
    assert_eq!(grown.accounting[0].task, reduced.accounting[0].task);
    assert_eq!(reduced.accounting[0].effective_revision, 3);
    assert!(
        reduced.accounting[0].compute_time.unwrap() >= grown.accounting[0].compute_time.unwrap()
    );
    assert_eq!(
        client
            .publish_control_settings(1, settings(4), client.vm_bounds(), 8, hosting(4096))
            .await
            .err(),
        Some(ServiceFailure::SettingsConflict)
    );
    assert_eq!(
        client
            .publish_control_settings(3, settings(4), client.vm_bounds(), 8, hosting(usize::MAX))
            .await
            .err(),
        Some(ServiceFailure::InvalidLimits)
    );
    assert_eq!(client.allocator_status(), reduced.allocator);
    ports.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(5), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result.outcome, TaskOutcome::Completed { .. }));
    assert_eq!(ports.reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 1);
    graceful(&mut owner).await;
}

#[tokio::test]
async fn boot_is_idle_and_original_opaque_ids_reach_real_ports() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let ports = FilePorts::new(directory.path(), "A", false);
    assert_eq!(ports.reads.load(Ordering::Acquire), 0);
    assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    let ticket = owner.client().submit(ports.input(), ports.clone()).unwrap();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.outcome,
        TaskOutcome::Completed {
            reply_ref: "msg:message A".into()
        }
    );
    assert!(receipt.withheld.is_empty());
    assert_eq!(ports.reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 1);
    // Another task uses this already-parked instance, never another root boot.
    let next = FilePorts::new(directory.path(), "B", false);
    let next_ticket = owner.client().submit(next.input(), next.clone()).unwrap();
    let next_receipt = tokio::time::timeout(Duration::from_secs(10), next_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        next_receipt.outcome,
        TaskOutcome::Completed {
            reply_ref: "msg:message B".into()
        }
    );
    graceful(&mut owner).await;
}

#[tokio::test]
async fn dropped_waiter_retains_actual_read_and_other_task_progress() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let ports = FilePorts::new(directory.path(), "A", true);
    let ticket = owner.client().submit(ports.input(), ports.clone()).unwrap();
    let control = ticket.control();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    drop(ticket);
    assert_eq!(
        control.stopped(Duration::from_millis(20)).await,
        Err(ServiceFailure::Deadline)
    );
    let other = FilePorts::new(directory.path(), "B", false);
    let ticket = owner.client().submit(other.input(), other.clone()).unwrap();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    assert_eq!(other.writes.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    ports.release.notify_one();
    let cancelled = tokio::time::timeout(Duration::from_secs(10), control.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cancelled.outcome,
        TaskOutcome::Failed {
            reason_kind: "task_cancelled".into()
        }
    );
    assert_eq!(cancelled.withheld.len(), 1);
    match &cancelled.withheld[0].answer {
        brassclaw_monty_host::HostAnswer::Return(value) => assert_eq!(
            value.as_str().unwrap(),
            std::fs::read_to_string(&ports.input).unwrap()
        ),
        _ => panic!("retain the actual completed read, not a simulated cancellation result"),
    }
    control.stopped(Duration::from_secs(1)).await.unwrap();
    assert!(!ports.reply.exists());
    graceful(&mut owner).await;
}

#[tokio::test]
async fn live_backlog_limits_preserve_debt_grow_past_channel_size_and_wake_closed_waiters() {
    use brassclaw_monty_host::{process::TaskSettings, service::AdmissionLimits};
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start_with_capacity(FILE_ROOT, 2).await;
    let client = owner.client();
    let identity = client.root_identity();
    let mut busy = Vec::new();
    for label in ["backlog-busy-a", "backlog-busy-b"] {
        let ports = FilePorts::new(directory.path(), label, true);
        let ticket = client.submit(ports.input(), ports.clone()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
            .await
            .unwrap();
        busy.push((ports, ticket));
    }
    let mut queued = Vec::new();
    for label in ["backlog-owned-a", "backlog-owned-b"] {
        let ports = FilePorts::new(directory.path(), label, false);
        let ticket = client.submit(ports.input(), ports.clone()).unwrap();
        queued.push((ports, ticket));
    }
    let before = client.admission_observation();
    assert_eq!(before.tasks, 2);
    assert!(before.bytes > 0);
    let settings = |revision| TaskSettings {
        revision,
        max_compute_time: Duration::from_secs(600),
        token_budgets_enabled: false,
    };
    let reduced = client
        .publish_hosting_settings(
            1,
            settings(2),
            client.vm_bounds(),
            8,
            AdmissionLimits {
                max_tasks: 1,
                max_bytes: before.bytes - 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(reduced.admission.tasks, 2);
    assert_eq!(reduced.admission.bytes, before.bytes);
    assert!(reduced.admission.over_capacity());
    assert_eq!(client.root_identity(), identity);
    let waiting = FilePorts::new(directory.path(), "backlog-waiter", false);
    assert!(matches!(
        client.submit(waiting.input(), waiting.clone()),
        Err(ServiceFailure::Backpressure)
    ));
    let mut abandoned = Box::pin(client.submit_when_available(waiting.input(), waiting.clone()));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut abandoned)
            .await
            .is_err()
    );
    drop(abandoned);
    assert_eq!(client.admission_observation(), reduced.admission);
    let waiter_client = client.clone();
    let waiter_ports = waiting.clone();
    let mut waiter = tokio::spawn(async move {
        waiter_client
            .submit_when_available(waiter_ports.input(), waiter_ports)
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut waiter)
            .await
            .is_err()
    );
    client
        .publish_hosting_settings(
            2,
            settings(3),
            client.vm_bounds(),
            8,
            AdmissionLimits {
                max_tasks: 70,
                max_bytes: before.bytes - 1,
            },
        )
        .await
        .unwrap();
    // Count growth alone cannot override byte ownership.
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut waiter)
            .await
            .is_err()
    );
    assert_eq!(client.admission_observation().bytes, before.bytes);
    client
        .publish_hosting_settings(
            3,
            settings(4),
            client.vm_bounds(),
            8,
            AdmissionLimits {
                max_tasks: 70,
                max_bytes: 1024 * 1024,
            },
        )
        .await
        .unwrap();
    let waiting_ticket = tokio::time::timeout(Duration::from_secs(2), waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    queued.push((waiting, waiting_ticket));
    for ordinal in 3..70 {
        let ports = FilePorts::new(
            directory.path(),
            &format!("backlog-growth-{ordinal}"),
            false,
        );
        let ticket = client.submit(ports.input(), ports.clone()).unwrap();
        queued.push((ports, ticket));
    }
    let full = client.admission_observation();
    assert_eq!(full.tasks, 70);
    assert!(!full.over_capacity());
    assert!(full.bytes > before.bytes);
    let closed_waiter = FilePorts::new(directory.path(), "backlog-close", false);
    let mut closed =
        Box::pin(client.submit_when_available(closed_waiter.input(), closed_waiter.clone()));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut closed)
            .await
            .is_err()
    );
    assert!(matches!(
        client
            .publish_hosting_settings(
                2,
                settings(5),
                client.vm_bounds(),
                8,
                AdmissionLimits {
                    max_tasks: 1,
                    max_bytes: 1
                }
            )
            .await,
        Err(ServiceFailure::SettingsConflict)
    ));
    assert_eq!(client.admission_observation(), full);
    client.close_admission();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), &mut closed)
            .await
            .unwrap(),
        Err(ServiceFailure::Closed)
    ));
    drop(closed);
    for (ports, ticket) in &queued {
        ticket.control().cancel();
        let receipt = tokio::time::timeout(Duration::from_secs(2), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(receipt.accounting.is_none());
        assert_eq!(
            receipt.outcome,
            TaskOutcome::Failed {
                reason_kind: "task_cancelled".into()
            }
        );
        assert_eq!(ports.reads.load(Ordering::Acquire), 0);
        assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    }
    assert_eq!(client.admission_observation().tasks, 0);
    assert_eq!(client.admission_observation().bytes, 0);
    assert_eq!(closed_waiter.reads.load(Ordering::Acquire), 0);
    for (ports, ticket) in &busy {
        ports.release.notify_one();
        let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.root, identity);
        assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
        assert_eq!(ports.reads.load(Ordering::Acquire), 1);
        assert_eq!(ports.writes.load(Ordering::Acquire), 1);
    }
    graceful(&mut owner).await;
}

#[tokio::test]
async fn queued_cancellation_passes_fifo_front_without_a_free_worker_or_capacity_growth() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start_with_capacity(FILE_ROOT, 2).await;
    let client = owner.client();
    let a = FilePorts::new(directory.path(), "busy-a", true);
    let a_ticket = client.submit(a.input(), a.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), a.started.notified())
        .await
        .unwrap();
    let b = FilePorts::new(directory.path(), "busy-b", true);
    let b_ticket = client.submit(b.input(), b.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), b.started.notified())
        .await
        .unwrap();
    let front = FilePorts::new(directory.path(), "queued-front", false);
    let front_ticket = client.submit(front.input(), front.clone()).unwrap();
    let tail = FilePorts::new(directory.path(), "queued-tail", false);
    let tail_ticket = client.submit(tail.input(), tail.clone()).unwrap();
    tail_ticket.control().cancel();
    let receipt = tokio::time::timeout(Duration::from_secs(2), tail_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.outcome,
        TaskOutcome::Failed {
            reason_kind: "task_cancelled".into()
        }
    );
    let queued_root = receipt.root;
    assert_eq!(queued_root.source_checksum(), boot(FILE_ROOT).checksum);
    assert!(receipt.accounting.is_none());
    assert!(receipt.withheld.is_empty());
    assert!(tail.fenced.load(Ordering::Acquire));
    assert_eq!(tail.reads.load(Ordering::Acquire), 0);
    assert!(!tail.reply.exists());
    assert_eq!(front.reads.load(Ordering::Acquire), 0);
    assert!(matches!(
        front_ticket
            .control()
            .stopped(Duration::from_millis(20))
            .await,
        Err(ServiceFailure::Deadline)
    ));
    let replacement = FilePorts::new(directory.path(), "queue-replacement", false);
    let replacement_ticket = client
        .submit(replacement.input(), replacement.clone())
        .unwrap();
    let excess = FilePorts::new(directory.path(), "queue-excess", false);
    assert!(matches!(
        client.submit(excess.input(), excess.clone()),
        Err(ServiceFailure::Backpressure)
    ));
    for ticket in [&front_ticket, &replacement_ticket] {
        ticket.control().cancel();
        let receipt = tokio::time::timeout(Duration::from_secs(2), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(receipt.accounting.is_none());
        assert_eq!(
            receipt.outcome,
            TaskOutcome::Failed {
                reason_kind: "task_cancelled".into()
            }
        );
    }
    a.release.notify_one();
    b.release.notify_one();
    for ticket in [&a_ticket, &b_ticket] {
        let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.root, queued_root);
        assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
        assert!(receipt.accounting.is_some());
    }
    graceful(&mut owner).await;
}

#[tokio::test]
async fn memory_pressure_holds_admission_without_blocking_controls_or_active_results() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start_with_capacity(FILE_ROOT, 1).await;
    let pid = owner.worker_process_id();
    let client = owner.client();
    let root = client.root_identity();
    let a = FilePorts::new(directory.path(), "pressure-active-a", true);
    let a_ticket = client.submit(a.input(), a.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), a.started.notified())
        .await
        .unwrap();
    let b = FilePorts::new(directory.path(), "pressure-active-b", true);
    let b_ticket = client.submit(b.input(), b.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), b.started.notified())
        .await
        .unwrap();
    let queued = FilePorts::new(directory.path(), "pressure-queued", false);
    let queued_ticket = client.submit(queued.input(), queued.clone()).unwrap();

    let mut abandoned = Box::pin(client.publish_memory_backpressure(0, true));
    assert!(matches!(
        futures::poll!(abandoned.as_mut()),
        std::task::Poll::Pending
    ));
    drop(abandoned);
    // FIFO control ownership retains the accepted pause after its waiter drops.
    assert_eq!(
        client.publish_memory_backpressure(0, false).await,
        Err(ServiceFailure::SettingsConflict)
    );
    assert!(client.memory_admission_policy().backpressure);
    assert_eq!(client.memory_admission_policy().revision, 1);
    a.release.notify_one();
    let completed = tokio::time::timeout(Duration::from_secs(10), a_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(completed.outcome, TaskOutcome::Completed { .. }));
    assert_eq!(completed.root, root);
    assert_eq!(queued.reads.load(Ordering::Acquire), 0);
    assert_eq!(
        queued_ticket
            .control()
            .stopped(Duration::from_millis(20))
            .await,
        Err(ServiceFailure::Deadline)
    );

    let waiting = FilePorts::new(directory.path(), "pressure-waiting", false);
    let mut pending = Box::pin(client.submit_when_available(waiting.input(), waiting.clone()));
    // The sole queue credit is held by queued_ticket, not silently expanded.
    assert!(
        tokio::time::timeout(Duration::from_millis(20), pending.as_mut())
            .await
            .is_err()
    );
    queued_ticket.control().cancel();
    let cancelled = tokio::time::timeout(Duration::from_secs(2), queued_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(cancelled.accounting.is_none());
    assert_eq!(
        cancelled.outcome,
        TaskOutcome::Failed {
            reason_kind: "task_cancelled".into()
        }
    );
    // Its credit is now available, but pressure still prevents VM admission.
    assert!(
        tokio::time::timeout(Duration::from_millis(20), pending.as_mut())
            .await
            .is_err()
    );
    assert_eq!(waiting.reads.load(Ordering::Acquire), 0);
    drop(pending);

    let current = client.live_task_settings().current();
    client
        .publish_settings(
            current.revision,
            brassclaw_monty_host::process::TaskSettings {
                revision: current.revision + 1,
                max_compute_time: Duration::from_secs(90),
                token_budgets_enabled: current.limits.token_budgets_enabled,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        client.live_task_settings().current().revision,
        current.revision + 1
    );
    let clear = client.publish_memory_backpressure(1, false).await.unwrap();
    assert!(!clear.backpressure);
    assert_eq!(clear.revision, 2);
    // Dropping the parked submission returned its credit. The original input
    // executes once after capacity returns, on the same root and worker process.
    let ticket = tokio::time::timeout(
        Duration::from_secs(2),
        client.submit_when_available(waiting.input(), waiting.clone()),
    )
    .await
    .unwrap()
    .unwrap();
    let completed = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.root, root);
    assert!(matches!(completed.outcome, TaskOutcome::Completed { .. }));
    assert_eq!(waiting.reads.load(Ordering::Acquire), 1);
    assert_eq!(waiting.writes.load(Ordering::Acquire), 1);
    assert_eq!(owner.worker_process_id(), pid);

    client.publish_memory_backpressure(2, true).await.unwrap();
    let closed = FilePorts::new(directory.path(), "pressure-closed", false);
    let mut pending = Box::pin(client.submit_when_available(closed.input(), closed.clone()));
    assert!(matches!(
        futures::poll!(pending.as_mut()),
        std::task::Poll::Pending
    ));
    client.close_admission();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), pending.as_mut())
            .await
            .unwrap(),
        Err(ServiceFailure::Closed)
    ));
    drop(pending);
    assert_eq!(closed.reads.load(Ordering::Acquire), 0);
    b.release.notify_one();
    let completed = tokio::time::timeout(Duration::from_secs(10), b_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(completed.outcome, TaskOutcome::Completed { .. }));
    graceful(&mut owner).await;
}

#[tokio::test]
async fn fatal_worker_exit_fences_and_retains_queued_inputs_before_held_io_finishes() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start_with_capacity(FILE_ROOT, 2).await;
    let client = owner.client();
    let a = FilePorts::new(directory.path(), "fatal-busy-a", true);
    let a_ticket = client.submit(a.input(), a.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), a.started.notified())
        .await
        .unwrap();
    let b = FilePorts::new(directory.path(), "fatal-busy-b", true);
    let b_ticket = client.submit(b.input(), b.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), b.started.notified())
        .await
        .unwrap();
    let c = FilePorts::new(directory.path(), "fatal-queued-c", false);
    let c_ticket = client.submit(c.input(), c.clone()).unwrap();
    let d = FilePorts::new(directory.path(), "fatal-queued-d", false);
    let d_ticket = client.submit(d.input(), d.clone()).unwrap();
    assert!(
        tokio::process::Command::new("/bin/kill")
            .args(["-KILL", &owner.worker_process_id().unwrap().to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    for ticket in [&c_ticket, &d_ticket] {
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), ticket.wait())
                .await
                .unwrap(),
            Err(ServiceFailure::Transport)
        ));
    }
    for ports in [&c, &d] {
        assert!(ports.fenced.load(Ordering::Acquire));
        assert_eq!(ports.reads.load(Ordering::Acquire), 0);
        assert!(!ports.reply.exists());
    }
    assert!(matches!(
        client.submit(c.input(), c.clone()),
        Err(ServiceFailure::Closed)
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), owner.join())
            .await
            .is_err()
    );
    a.release.notify_one();
    b.release.notify_one();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.failure, Some(ServiceFailure::Transport));
    assert!(exit.pending_admission.is_none());
    assert_eq!(exit.queued_admissions.len(), 2);
    assert_eq!(exit.queued_admissions[0].input, c.expected);
    assert_eq!(exit.queued_admissions[1].input, d.expected);
    assert_eq!(exit.tasks.len(), 2);
    for ticket in [&a_ticket, &b_ticket] {
        assert!(matches!(
            ticket.wait().await,
            Err(ServiceFailure::Transport)
        ));
    }
    let transport = exit.transport.unwrap();
    assert_eq!(transport.kind, StopKind::TransportFailed);
    assert!(transport.exit_status.is_some());
    assert!(transport.containment_error.is_none());
    assert!(transport.reap_error.is_none());
}

#[tokio::test]
async fn actual_fatal_admission_retains_issued_handle_and_original_work() {
    let directory = tempfile::tempdir().unwrap();
    let source = "import asyncio\nasync def worker(i):\n    task = await host.await_next_task(i)\n    if task is not None:\n        raise RuntimeError('fatal root')\nawait asyncio.gather(*[worker(i) for i in range(worker_count)])";
    let mut owner = start(source).await;
    let ports = FilePorts::new(directory.path(), "A", false);
    let ticket = owner.client().submit(ports.input(), ports.clone()).unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap(),
        Err(ServiceFailure::Transport)
    ));
    let exit = owner.join().await.unwrap();
    assert_eq!(exit.failure, Some(ServiceFailure::Transport));
    let pending = exit
        .pending_admission
        .expect("retain admission even before a first port");
    assert_eq!(pending.input, ports.expected);
    assert_eq!(exit.failed_exchanges.len(), 1);
    let failed = exit.failed_exchanges[0].outcome.as_ref().err().unwrap();
    assert!(failed.snapshot.as_ref().unwrap().admitted_task.is_some());
    assert_eq!(failed.snapshot.as_ref().unwrap().task_accounting.len(), 1);
    assert!(ports.fenced.load(Ordering::Acquire));
    assert_eq!(ports.reads.load(Ordering::Acquire), 0);
    assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    assert_eq!(exit.transport.unwrap().kind, StopKind::InstanceFailed);
}

#[tokio::test]
async fn port_panic_contains_instance_and_preserves_task_for_reconciliation() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let mut ports = FilePorts::new(directory.path(), "A", false);
    Arc::get_mut(&mut ports).unwrap().panic_after_read = true;
    let ticket = owner.client().submit(ports.input(), ports.clone()).unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap(),
        Err(ServiceFailure::InvalidPortResult)
    ));
    let exit = owner.join().await.unwrap();
    assert_eq!(exit.failure, Some(ServiceFailure::InvalidPortResult));
    assert_eq!(exit.tasks.len(), 1);
    assert_eq!(ports.reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    assert!(ports.fenced.load(Ordering::Acquire));
    assert!(!exit.last_snapshot.outstanding.is_empty());
    assert!(exit.transport.unwrap().exit_status.is_some());
}

#[tokio::test]
async fn settings_publication_preserves_active_task_consumption_and_rejects_stale_edits() {
    use brassclaw_monty_host::process::TaskSettings;
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let live = client.live_task_settings();
    let ports = FilePorts::new(directory.path(), "live-settings", true);
    let ticket = client.submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    let first = client
        .publish_settings(
            1,
            TaskSettings {
                revision: 2,
                max_compute_time: Duration::from_secs(7200),
                token_budgets_enabled: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(first.effective_settings.revision, 2);
    assert_eq!(
        first.effective_settings.max_compute_time,
        Duration::from_secs(7200)
    );
    assert_eq!(live.current().revision, 2);
    assert_eq!(first.accounting.len(), 1);
    let usage = first.accounting[0].compute_time.unwrap();
    assert!(usage > Duration::ZERO);
    assert_eq!(first.accounting[0].effective_revision, 2);
    let stale = client
        .publish_settings(
            1,
            TaskSettings {
                revision: 3,
                max_compute_time: Duration::from_secs(600),
                token_budgets_enabled: false,
            },
        )
        .await;
    assert!(matches!(stale, Err(ServiceFailure::SettingsConflict)));
    assert_eq!(live.current().revision, 2);
    let second = client
        .publish_settings(
            2,
            TaskSettings {
                revision: 3,
                max_compute_time: Duration::from_secs(1),
                token_budgets_enabled: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.effective_settings.revision, 3);
    assert_eq!(
        second.effective_settings.max_compute_time,
        Duration::from_secs(1)
    );
    assert_eq!(live.current().revision, 3);
    assert_eq!(
        second.accounting[0].compute_time.unwrap(),
        usage,
        "host waiting and settings transport do not debit or reset task compute time"
    );
    ports.release.notify_one();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    let accounting = receipt.accounting.as_ref().unwrap();
    assert_eq!(accounting.effective_revision, 3);
    assert!(accounting.compute_time.unwrap() >= usage);
    assert!(accounting.failure.is_none());
    graceful(&mut owner).await;
}

#[tokio::test]
async fn owned_heap_edits_reject_unsafe_limits_and_survive_dropped_waiter_during_real_io() {
    use brassclaw_monty_host::heap::HeapSettings;
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let initial = client.heap_observation();
    assert!(initial.vm_live_bytes > 0);
    assert_eq!(initial.status.effective.unwrap().revision, 1);
    let ports = FilePorts::new(directory.path(), "live-heap", true);
    let ticket = client.submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    assert_eq!(
        client
            .publish_heap(
                1,
                HeapSettings {
                    revision: 2,
                    max_vm_bytes: 1
                },
                false
            )
            .await,
        Err(ServiceFailure::UnsafeHeapReduction)
    );
    assert_eq!(client.heap_observation().status, initial.status);
    assert_eq!(
        client
            .publish_heap(
                1,
                HeapSettings {
                    revision: 2,
                    max_vm_bytes: 0
                },
                false
            )
            .await,
        Err(ServiceFailure::InvalidLimits)
    );
    let mut abandoned = Box::pin(client.publish_heap(
        1,
        HeapSettings {
            revision: 2,
            max_vm_bytes: 32 * 1024 * 1024,
        },
        false,
    ));
    assert!(matches!(
        futures::poll!(abandoned.as_mut()),
        std::task::Poll::Pending
    ));
    drop(abandoned);
    // FIFO admission of control requests establishes that revision 2 was
    // actually applied despite its waiter being dropped, before revision 3.
    let third = client
        .publish_heap(
            2,
            HeapSettings {
                revision: 3,
                max_vm_bytes: 16 * 1024 * 1024,
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(third.status.effective.unwrap().revision, 3);
    assert_eq!(client.heap_observation(), third);
    assert_eq!(client.live_task_settings().current().revision, 1);
    assert_eq!(
        ports.writes.load(Ordering::Acquire),
        0,
        "waiting I/O was not replayed or completed by the edit"
    );
    ports.release.notify_one();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    assert_eq!(ports.reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 1);
    graceful(&mut owner).await;
}

#[tokio::test]
async fn normal_completion_releases_child_heap_before_receipt_and_keeps_global_root() {
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let initial = client.heap_observation();
    let mut ports = FilePorts::new(directory.path(), "completed-memory", true);
    Arc::get_mut(&mut ports).unwrap().retain_child_heap = true;
    let ticket = client.submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    let occupied = ports.retained_vm_bytes.load(Ordering::Acquire);
    assert!(occupied >= initial.vm_live_bytes + 2 * 1024 * 1024);
    ports.release.notify_one();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    let released = client.heap_observation();
    assert!(
        released.vm_live_bytes + 2 * 1024 * 1024 <= occupied,
        "completion must publish the post-release observation, including while idle"
    );
    assert_eq!(released.status, initial.status);
    assert_eq!(ports.reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 1);
    // Retain the first ticket/receipt to catch service-side context leaks. A
    // completed caller retaining its receipt must not retain the child VM.
    let next = FilePorts::new(directory.path(), "after-completion", false);
    let next_ticket = client.submit(next.input(), next.clone()).unwrap();
    let next_receipt = tokio::time::timeout(Duration::from_secs(10), next_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_receipt.root, receipt.root);
    assert!(matches!(
        next_receipt.outcome,
        TaskOutcome::Completed { .. }
    ));
    assert_eq!(next.reads.load(Ordering::Acquire), 1);
    assert_eq!(next.writes.load(Ordering::Acquire), 1);
    graceful(&mut owner).await;
}

#[tokio::test]
async fn live_bounds_reach_retained_children_without_resetting_feeds_or_recipe_locals() {
    use brassclaw_monty_host::process::TaskSettings;
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let pid = owner.worker_process_id();
    let client = owner.client();
    let mut ports = FilePorts::new(directory.path(), "live-child-limits", false);
    let port = Arc::get_mut(&mut ports).unwrap();
    port.retain_child_heap = true;
    port.resume_child_after_edit = true;
    let ticket = client.submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    let mut bounds = client.vm_bounds();
    bounds.max_feeds = 2;
    bounds.execution_slice = Duration::from_millis(1);
    let update = client
        .publish_runtime_settings(
            1,
            TaskSettings {
                revision: 2,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            bounds,
        )
        .await
        .unwrap();
    assert_eq!(client.vm_bounds(), bounds);
    assert_eq!(update.accounting.len(), 1);
    assert!(update.accounting[0].compute_time.unwrap() > Duration::ZERO);
    assert_eq!(owner.worker_process_id(), pid);
    let mut invalid = bounds;
    invalid.max_source_bytes = 1;
    assert!(matches!(
        client
            .publish_runtime_settings(
                2,
                TaskSettings {
                    revision: 3,
                    max_compute_time: Duration::from_secs(30),
                    token_budgets_enabled: false,
                },
                invalid
            )
            .await,
        Err(ServiceFailure::InvalidLimits)
    ));
    assert_eq!(client.live_task_settings().current().revision, 2);
    assert_eq!(client.vm_bounds(), bounds);
    ports.release.notify_one();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    assert_eq!(
        std::fs::read(&ports.reply).unwrap(),
        std::fs::read(&ports.input).unwrap()
    );
    graceful(&mut owner).await;
}

#[tokio::test]
async fn live_child_capacity_drains_owned_contexts_and_keeps_their_original_state() {
    use brassclaw_monty_host::process::{
        ProcessFailure, RecipeCommand, RecipeEvent, TaskSettings, WorkerCommand,
    };
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let identity = client.root_identity();
    let (opened_tx, mut opened_rx) = tokio::sync::mpsc::channel(2);
    let mut children = Vec::new();
    for label in ["context-capacity-a", "context-capacity-b"] {
        let mut ports = FilePorts::new(directory.path(), label, false);
        let writable = Arc::get_mut(&mut ports).unwrap();
        writable.retain_child_heap = true;
        writable.resume_child_after_edit = true;
        writable.child_opened = Some(opened_tx.clone());
        let ticket = client.submit(ports.input(), ports.clone()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
            .await
            .unwrap();
        children.push((ports, ticket));
    }
    let HeldChild {
        task,
        parent,
        transport,
    } = opened_rx.recv().await.unwrap();
    let before = client.recipe_context_capacity();
    assert_eq!(before.active, 2);
    let mut bounds = client.vm_bounds();
    bounds.max_feeds = 2;
    let reduced = client
        .publish_execution_settings(
            1,
            TaskSettings {
                revision: 2,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            bounds,
            1,
        )
        .await
        .unwrap();
    assert_eq!(reduced.recipe_context_capacity.limit, 1);
    assert_eq!(reduced.recipe_context_capacity.active, 2);
    assert_eq!(reduced.accounting.len(), 2);
    assert!(reduced.accounting.iter().all(|account| {
        account
            .compute_time
            .is_some_and(|elapsed| elapsed > Duration::ZERO)
    }));
    let denied = transport
        .try_submit(WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task,
                parent: Some(parent),
            },
        })
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(matches!(
        denied.outcome,
        Err(brassclaw_monty_host::process::ProcessError {
            kind: ProcessFailure::Vm(brassclaw_monty_host::VmFailure::ResourceLimit),
            ..
        })
    ));
    assert_eq!(client.recipe_context_capacity().active, 2);
    let grown = client
        .publish_execution_settings(
            2,
            TaskSettings {
                revision: 3,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            bounds,
            3,
        )
        .await
        .unwrap();
    assert_eq!(grown.recipe_context_capacity.limit, 3);
    let opened = transport
        .try_submit(WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task,
                parent: Some(parent),
            },
        })
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap();
    assert!(matches!(opened.recipe, Some(RecipeEvent::Opened { .. })));
    assert_eq!(client.recipe_context_capacity().active, 3);
    for (ports, ticket) in children {
        ports.release.notify_one();
        let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
        assert_eq!(receipt.root, identity);
        assert!(ports.child_resumed.load(Ordering::Acquire));
        assert_eq!(
            std::fs::read(&ports.reply).unwrap(),
            std::fs::read(&ports.input).unwrap()
        );
        assert_eq!(ports.writes.load(Ordering::Acquire), 1);
    }
    assert_eq!(client.recipe_context_capacity().active, 0);
    assert_eq!(client.root_identity(), identity);
    graceful(&mut owner).await;
}

#[tokio::test]
async fn reduced_value_limit_preserves_completed_host_result_and_fails_only_its_task() {
    use brassclaw_monty_host::{HostAnswer, process::TaskSettings};
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let pid = owner.worker_process_id();
    let client = owner.client();
    let ports = FilePorts::new(directory.path(), "large-result", true);
    let content = "actual file payload Ü".repeat(512);
    std::fs::write(&ports.input, &content).unwrap();
    let ticket = client.submit(ports.input(), ports.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ports.started.notified())
        .await
        .unwrap();
    let mut bounds = client.vm_bounds();
    bounds.max_value_bytes = 1024;
    client
        .publish_runtime_settings(
            1,
            TaskSettings {
                revision: 2,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            bounds,
        )
        .await
        .unwrap();
    ports.release.notify_one();
    let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Failed { .. }));
    assert_eq!(receipt.withheld.len(), 1);
    assert!(
        matches!(&receipt.withheld[0].answer, HostAnswer::Return(Value::String(value)) if value == &content)
    );
    assert_eq!(ports.reads.load(Ordering::Acquire), 1);
    assert_eq!(ports.writes.load(Ordering::Acquire), 0);
    let next = FilePorts::new(directory.path(), "bounded-next", false);
    let next_ticket = client.submit(next.input(), next.clone()).unwrap();
    let next_receipt = tokio::time::timeout(Duration::from_secs(10), next_ticket.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        next_receipt.outcome,
        TaskOutcome::Completed { .. }
    ));
    assert_eq!(next_receipt.root, receipt.root);
    assert_eq!(owner.worker_process_id(), pid);
    graceful(&mut owner).await;
}

#[tokio::test]
async fn queued_input_is_rechecked_after_live_reduction_without_truncation_or_instance_failure() {
    use brassclaw_monty_host::process::TaskSettings;
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let first = FilePorts::new(directory.path(), "queued-first", true);
    let second = FilePorts::new(directory.path(), "queued-second", true);
    let a = client.submit(first.input(), first.clone()).unwrap();
    let b = client.submit(second.input(), second.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        first.started.notified().await;
        second.started.notified().await;
    })
    .await
    .unwrap();
    let mut large = FilePorts::new(directory.path(), "queued-large", false);
    Arc::get_mut(&mut large).unwrap().expected["user_input"] = json!("queued original".repeat(256));
    let queued = client.submit(large.input(), large.clone()).unwrap();
    let mut bounds = client.vm_bounds();
    bounds.max_value_bytes = 1024;
    client
        .publish_runtime_settings(
            1,
            TaskSettings {
                revision: 2,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            bounds,
        )
        .await
        .unwrap();
    first.release.notify_one();
    second.release.notify_one();
    for ticket in [a, b] {
        let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    }
    let rejected = tokio::time::timeout(Duration::from_secs(10), queued.wait())
        .await
        .unwrap();
    assert!(matches!(rejected, Err(ServiceFailure::InvalidInput)));
    assert_eq!(large.reads.load(Ordering::Acquire), 0);
    assert_eq!(large.writes.load(Ordering::Acquire), 0);
    let next = FilePorts::new(directory.path(), "queue-next", false);
    let receipt = client
        .submit(next.input(), next)
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    graceful(&mut owner).await;
}

#[tokio::test]
async fn live_settings_capacity_retains_accepted_edits_and_grows_beyond_eight() {
    use brassclaw_monty_host::{
        heap::HeapSettings,
        process::TaskSettings,
        service::{HeapCommitOutcome, HeapObservation, ServiceClient, ServiceHostingLimits},
    };
    async fn held_commit(
        client: ServiceClient,
        path: std::path::PathBuf,
    ) -> (
        Arc<Notify>,
        tokio::task::JoinHandle<Result<HeapObservation, ServiceFailure>>,
    ) {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let expected = client.heap_observation().status.desired_revision();
        let next = HeapSettings {
            revision: expected + 1,
            max_vm_bytes: client
                .heap_observation()
                .status
                .effective
                .unwrap()
                .max_vm_bytes
                + 1024,
        };
        let publication = tokio::spawn({
            let entered = entered.clone();
            let release = release.clone();
            async move {
                client
                    .publish_heap_transaction(
                        expected,
                        next,
                        Box::pin(async move {
                            entered.notify_one();
                            release.notified().await;
                            use std::io::Write;
                            match std::fs::OpenOptions::new()
                                .create_new(true)
                                .write(true)
                                .open(path)
                            {
                                Ok(mut file) => match file
                                    .write_all(next.max_vm_bytes.to_string().as_bytes())
                                    .and_then(|()| file.sync_all())
                                {
                                    Ok(()) => HeapCommitOutcome::Committed,
                                    Err(_) => HeapCommitOutcome::Unknown,
                                },
                                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                                    HeapCommitOutcome::Rejected
                                }
                                Err(_) => HeapCommitOutcome::Unknown,
                            }
                        }),
                    )
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        (release, publication)
    }
    let directory = tempfile::tempdir().unwrap();
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let identity = client.root_identity();
    assert_eq!(client.settings_capacity().limit, 8);
    let first_path = directory.path().join("first-committed-settings");
    let (release, commit) = held_commit(client.clone(), first_path.clone()).await;
    let hosting = |limit| ServiceHostingLimits {
        adapter_reserve_bytes: None,
        admission: client.admission_observation().limits,
        max_pending_settings: limit,
        max_retained_attempts: 256,
        actor: None,
        deadlines: None,
    };
    let settings = |revision| TaskSettings {
        revision,
        max_compute_time: Duration::from_secs(600),
        token_budgets_enabled: false,
    };
    let mut reduction = Box::pin(client.publish_control_settings(
        1,
        settings(2),
        client.vm_bounds(),
        8,
        hosting(2),
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut reduction)
            .await
            .is_err()
    );
    let mut abandoned = Vec::new();
    for expected_pending in 3..=8 {
        let mut edit = Box::pin(client.publish_memory_backpressure(0, false));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut edit)
                .await
                .is_err()
        );
        assert_eq!(client.settings_capacity().pending, expected_pending);
        abandoned.push(edit);
    }
    assert_eq!(
        client.publish_memory_backpressure(0, false).await,
        Err(ServiceFailure::Backpressure)
    );
    drop(abandoned);
    assert_eq!(
        client.settings_capacity().pending,
        8,
        "dropped waiters retain accepted edits"
    );
    release.notify_one();
    let committed = commit.await.unwrap().unwrap();
    let reduced = tokio::time::timeout(Duration::from_secs(2), reduction)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reduced.settings_capacity.limit, 2);
    assert_eq!(reduced.settings_capacity.pending, 7);
    assert!(reduced.settings_capacity.over_capacity());
    tokio::time::timeout(Duration::from_secs(2), async {
        while client.settings_capacity().pending != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        client.memory_admission_policy().revision,
        1,
        "accepted first edit survived; stale successors failed CAS"
    );
    assert_eq!(
        std::fs::read_to_string(first_path).unwrap(),
        committed.status.effective.unwrap().max_vm_bytes.to_string()
    );
    let grown = client
        .publish_control_settings(2, settings(3), client.vm_bounds(), 8, hosting(20))
        .await
        .unwrap();
    assert_eq!(grown.settings_capacity.limit, 20);
    assert_eq!(client.root_identity(), identity);
    let second_path = directory.path().join("second-committed-settings");
    let (release, commit) = held_commit(client.clone(), second_path.clone()).await;
    let mut queued = Vec::new();
    for expected_pending in 2..=20 {
        let mut edit = Box::pin(client.publish_memory_backpressure(1, false));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut edit)
                .await
                .is_err()
        );
        assert_eq!(client.settings_capacity().pending, expected_pending);
        queued.push(edit);
    }
    assert_eq!(
        client.publish_memory_backpressure(1, false).await,
        Err(ServiceFailure::Backpressure)
    );
    owner.request_shutdown();
    assert_eq!(
        client.publish_memory_backpressure(1, false).await,
        Err(ServiceFailure::Closed)
    );
    release.notify_one();
    let committed = commit.await.unwrap().unwrap();
    for edit in queued {
        assert_eq!(edit.await, Err(ServiceFailure::Closed));
    }
    let exit = owner.join().await.unwrap();
    assert!(exit.failure.is_none());
    assert_eq!(client.settings_capacity().pending, 0);
    assert_eq!(client.memory_admission_policy().revision, 1);
    assert_eq!(client.root_identity(), identity);
    assert_eq!(
        std::fs::read_to_string(second_path).unwrap(),
        committed.status.effective.unwrap().max_vm_bytes.to_string()
    );
    assert_eq!(exit.transport.unwrap().kind, StopKind::Graceful);
}

#[tokio::test]
async fn manual_heap_commit_barrier_holds_direct_child_execution_and_rolls_back_safely() {
    use brassclaw_monty_host::{heap::HeapSettings, service::HeapCommitOutcome};
    let directory = tempfile::tempdir().unwrap();
    let mut root = boot(FILE_ROOT);
    root.aliases.insert("read_file".into());
    root.bounds.values.max_feeds = 2;
    let mut owner = ServiceOwner::start(
        worker(),
        root,
        limits(),
        ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: 4 * 1024 * 1024,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: 4 * 1024 * 1024,
        },
        2,
    )
    .await
    .unwrap();
    let client = owner.client();
    let identity = client.root_identity();
    let initial = client.heap_observation().status.effective.unwrap();
    for (label, committed) in [("rollback", false), ("commit", true)] {
        let mut ports = FilePorts::new(directory.path(), label, false);
        let port = Arc::get_mut(&mut ports).unwrap();
        port.retain_child_heap = true;
        port.resume_child_after_edit = true;
        let task = client.submit(ports.input(), ports.clone()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), ports.started.notified())
            .await
            .unwrap();
        if !committed {
            let expected = client.heap_observation().status.desired_revision();
            client
                .publish_heap(
                    expected,
                    HeapSettings {
                        revision: expected + 1,
                        max_vm_bytes: 1,
                    },
                    true,
                )
                .await
                .unwrap();
            assert!(client.heap_observation().status.pending_reduction);
        }
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let expected = client.heap_observation().status.desired_revision();
        let next = HeapSettings {
            revision: expected + 1,
            max_vm_bytes: initial.max_vm_bytes + 8 * 1024 * 1024,
        };
        let commit_path = directory.path().join(format!("heap-config-{label}"));
        if !committed {
            std::fs::write(&commit_path, "existing configuration").unwrap();
        }
        let publication = tokio::spawn({
            let commit_path = commit_path.clone();
            let client = client.clone();
            let entered = entered.clone();
            let release = release.clone();
            async move {
                client
                    .publish_heap_transaction(
                        expected,
                        next,
                        Box::pin(async move {
                            entered.notify_one();
                            release.notified().await;
                            use std::io::Write;
                            match std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(commit_path)
                            {
                                Ok(mut file) => {
                                    if file
                                        .write_all(next.max_vm_bytes.to_string().as_bytes())
                                        .and_then(|()| file.sync_all())
                                        .is_ok()
                                    {
                                        HeapCommitOutcome::Committed
                                    } else {
                                        HeapCommitOutcome::Unknown
                                    }
                                }
                                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                                    HeapCommitOutcome::Rejected
                                }
                                Err(_) => HeapCommitOutcome::Unknown,
                            }
                        }),
                    )
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        ports.release.notify_one();
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(
            !ports.child_resumed.load(Ordering::Acquire),
            "direct child must stay queued until commit settles"
        );
        assert_eq!(
            client
                .heap_observation()
                .status
                .effective
                .unwrap()
                .max_vm_bytes,
            initial.max_vm_bytes
        );
        if committed {
            publication.abort();
        } // accepted operation retains ownership
        release.notify_one();
        if committed {
            assert!(publication.await.unwrap_err().is_cancelled());
        } else {
            assert_eq!(
                publication.await.unwrap(),
                Err(ServiceFailure::SettingsConflict)
            );
            let heap = client.heap_observation().status;
            assert!(heap.pending_reduction);
            assert_eq!(heap.desired.unwrap().max_vm_bytes, 1);
            assert_eq!(heap.effective.unwrap().max_vm_bytes, initial.max_vm_bytes);
            // Restore the operator's finite value before the next admission.
            let expected = heap.desired_revision();
            client
                .publish_heap(
                    expected,
                    HeapSettings {
                        revision: expected + 1,
                        max_vm_bytes: initial.max_vm_bytes,
                    },
                    false,
                )
                .await
                .unwrap();
        }
        let receipt = tokio::time::timeout(Duration::from_secs(2), task.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
        assert!(ports.child_resumed.load(Ordering::Acquire));
        assert_eq!(ports.reads.load(Ordering::Acquire), 1);
        assert_eq!(ports.writes.load(Ordering::Acquire), 1);
        assert_eq!(client.root_identity(), identity);
        if !committed {
            assert_eq!(
                client
                    .heap_observation()
                    .status
                    .effective
                    .unwrap()
                    .max_vm_bytes,
                initial.max_vm_bytes
            );
        } else {
            assert_eq!(
                std::fs::read_to_string(commit_path).unwrap(),
                next.max_vm_bytes.to_string()
            );
            assert_eq!(client.heap_observation().status.effective, Some(next));
        }
    }
    // Unsafe reductions must not even poll the persistence future.
    let polled = Arc::new(AtomicBool::new(false));
    let expected = client.heap_observation().status.desired_revision();
    let receipt = client
        .publish_heap_transaction(
            expected,
            HeapSettings {
                revision: expected + 1,
                max_vm_bytes: 1,
            },
            Box::pin({
                let polled = polled.clone();
                async move {
                    polled.store(true, Ordering::Release);
                    HeapCommitOutcome::Committed
                }
            }),
        )
        .await;
    assert_eq!(receipt, Err(ServiceFailure::UnsafeHeapReduction));
    assert!(!polled.load(Ordering::Acquire));
    owner.request_shutdown();
    let exit = owner.join().await.unwrap();
    assert!(exit.failure.is_none());
    assert_eq!(exit.transport.unwrap().kind, StopKind::Graceful);
}
#[tokio::test]
async fn live_retention_capacity_preserves_owned_credits_and_grows_beyond_256() {
    use brassclaw_monty_host::{process::TaskSettings, service::ServiceHostingLimits};
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let root = client.root_identity();
    assert_eq!(client.retained_attempts().limit, 256);
    let mut retained = (0..256)
        .map(|_| client.try_retain_attempt().unwrap())
        .collect::<Vec<_>>();
    assert!(matches!(
        client.try_retain_attempt(),
        Err(ServiceFailure::Backpressure)
    ));
    let publish = |expected, revision, limit| {
        let client = client.clone();
        async move {
            client
                .publish_control_settings(
                    expected,
                    TaskSettings {
                        revision,
                        max_compute_time: Duration::from_secs(600),
                        token_budgets_enabled: false,
                    },
                    client.vm_bounds(),
                    8,
                    ServiceHostingLimits {
                        adapter_reserve_bytes: None,
                        admission: client.admission_observation().limits,
                        max_pending_settings: client.settings_capacity().limit,
                        max_retained_attempts: limit,
                        actor: None,
                        deadlines: None,
                    },
                )
                .await
                .unwrap()
        }
    };
    let grown = publish(1, 2, 257).await;
    assert_eq!(grown.effective_settings.revision, 2);
    assert_eq!(grown.retained_attempts.limit, 257);
    retained.push(client.try_retain_attempt().unwrap());
    assert_eq!(client.retained_attempts().retained, 257);
    let reduced = publish(2, 3, 1).await;
    assert!(reduced.retained_attempts.over_capacity());
    assert_eq!(reduced.retained_attempts.retained, 257);
    assert_eq!(client.live_task_settings().current().revision, 3);
    assert!(matches!(
        client.try_retain_attempt(),
        Err(ServiceFailure::Backpressure)
    ));
    retained.truncate(1);
    assert_eq!(client.retained_attempts().retained, 1);
    assert!(matches!(
        client.try_retain_attempt(),
        Err(ServiceFailure::Backpressure)
    ));
    retained.clear();
    let final_credit = client.try_retain_attempt().unwrap();
    assert_eq!(client.root_identity(), root);
    graceful(&mut owner).await;
    assert!(matches!(
        client.try_retain_attempt(),
        Err(ServiceFailure::Closed)
    ));
    assert_eq!(client.retained_attempts().retained, 1);
    drop(final_credit);
    assert_eq!(client.retained_attempts().retained, 0);
}

#[tokio::test]
async fn actor_policy_is_published_with_the_worker_settings_revision() {
    use brassclaw_monty_host::{process::TaskSettings, service::ServiceHostingLimits};
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let root = client.root_identity();
    let initial = client.actor_capacity().unwrap().limits;
    let limits = ActorLimits {
        max_unclaimed: 2048,
        max_control_unclaimed: 2049,
        ..initial
    };
    let hosting = |actor| ServiceHostingLimits {
        adapter_reserve_bytes: None,
        admission: client.admission_observation().limits,
        max_pending_settings: client.settings_capacity().limit,
        max_retained_attempts: client.retained_attempts().limit,
        actor: Some(actor),
        deadlines: None,
    };
    let settings = TaskSettings {
        revision: 2,
        max_compute_time: Duration::from_secs(600),
        token_budgets_enabled: false,
    };
    let invalid = ActorLimits {
        max_control_reserved_frame_bytes: 1,
        ..limits
    };
    assert!(matches!(
        client
            .publish_control_settings(1, settings, client.vm_bounds(), 8, hosting(invalid))
            .await,
        Err(ServiceFailure::InvalidLimits)
    ));
    assert_eq!(client.live_task_settings().current().revision, 1);
    assert_eq!(client.actor_capacity().unwrap().limits, initial);
    let receipt = client
        .publish_control_settings(1, settings, client.vm_bounds(), 8, hosting(limits))
        .await
        .unwrap();
    assert_eq!(receipt.effective_settings.revision, 2);
    assert_eq!(receipt.actor_capacity.limits, limits);
    assert_eq!(client.actor_capacity().unwrap().limits, limits);
    assert_eq!(client.live_task_settings().current().revision, 2);
    assert!(matches!(
        client
            .publish_control_settings(1, settings, client.vm_bounds(), 8, hosting(initial))
            .await,
        Err(ServiceFailure::SettingsConflict)
    ));
    assert_eq!(client.actor_capacity().unwrap().limits, limits);
    let settings = TaskSettings {
        revision: 3,
        ..settings
    };
    let reduced = ActorLimits {
        max_unclaimed: 1,
        max_control_unclaimed: 1,
        ..initial
    };
    let receipt = client
        .publish_control_settings(2, settings, client.vm_bounds(), 8, hosting(reduced))
        .await
        .unwrap();
    assert_eq!(receipt.actor_capacity.limits, reduced);
    assert_eq!(client.root_identity(), root);
    graceful(&mut owner).await;
}

#[tokio::test]
async fn live_hosting_deadlines_share_revision_and_preserve_global_root() {
    use brassclaw_monty_host::{
        process::TaskSettings, service::ServiceHostingLimits, transport_actor::HostingDeadlines,
    };
    let mut owner = start(FILE_ROOT).await;
    let client = owner.client();
    let root = client.root_identity();
    let initial = client.hosting_deadlines().unwrap();
    let policy = |deadlines| ServiceHostingLimits {
        adapter_reserve_bytes: None,
        admission: client.admission_observation().limits,
        max_pending_settings: client.settings_capacity().limit,
        max_retained_attempts: client.retained_attempts().limit,
        actor: None,
        deadlines: Some(deadlines),
    };
    let settings = TaskSettings {
        revision: 2,
        max_compute_time: Duration::from_secs(600),
        token_budgets_enabled: false,
    };
    let grown = HostingDeadlines {
        startup_timeout: Duration::from_secs(2),
        response_timeout: Duration::from_secs(8),
    };
    let mut unsafe_slice = client.vm_bounds();
    unsafe_slice.execution_slice = initial.response_timeout;
    // A bigger future deadline does not extend already accepted old requests.
    assert!(matches!(
        client
            .publish_control_settings(1, settings, unsafe_slice, 8, policy(grown))
            .await,
        Err(ServiceFailure::Backpressure)
    ));
    assert_eq!(client.hosting_deadlines().unwrap(), initial);
    assert_eq!(client.live_task_settings().current().revision, 1);
    let receipt = client
        .publish_control_settings(1, settings, client.vm_bounds(), 8, policy(grown))
        .await
        .unwrap();
    assert_eq!(receipt.effective_settings.revision, 2);
    assert_eq!(receipt.hosting_deadlines, grown);
    assert_eq!(client.hosting_deadlines().unwrap(), grown);
    assert!(matches!(
        client
            .publish_control_settings(1, settings, client.vm_bounds(), 8, policy(initial))
            .await,
        Err(ServiceFailure::InvalidLimits)
    ));
    assert!(matches!(
        client
            .publish_control_settings(
                1,
                settings,
                client.vm_bounds(),
                8,
                policy(HostingDeadlines {
                    startup_timeout: Duration::from_secs(1),
                    response_timeout: Duration::from_secs(4)
                })
            )
            .await,
        Err(ServiceFailure::SettingsConflict)
    ));
    // The fixture's original 10 s startup / 5 s IPC pair is a legacy direct
    // transport boot; new coordinated settings require the safe relationship.
    assert_eq!(client.hosting_deadlines().unwrap(), grown);
    let settings = TaskSettings {
        revision: 3,
        ..settings
    };
    let reduced = HostingDeadlines {
        response_timeout: Duration::from_secs(3),
        ..grown
    };
    let receipt = client
        .publish_control_settings(2, settings, client.vm_bounds(), 8, policy(reduced))
        .await
        .unwrap();
    assert_eq!(receipt.hosting_deadlines, reduced);
    assert_eq!(client.live_task_settings().current().revision, 3);
    assert_eq!(client.root_identity(), root);
    // The actor now publishes Rust inside the worker ACK boundary. Validate
    // a combined slice/deadline reduction against the ACK's new bounds, not
    // the still-unpublished watch containing the old two-second slice.
    let mut large_slice = client.vm_bounds();
    large_slice.execution_slice = Duration::from_secs(2);
    let settings = TaskSettings {
        revision: 4,
        ..settings
    };
    client
        .publish_control_settings(3, settings, large_slice, 8, policy(reduced))
        .await
        .unwrap();
    let mut small_slice = large_slice;
    small_slice.execution_slice = Duration::from_millis(5);
    let shorter = HostingDeadlines {
        startup_timeout: Duration::from_millis(100),
        response_timeout: Duration::from_secs(1),
    };
    let settings = TaskSettings {
        revision: 5,
        ..settings
    };
    let receipt = client
        .publish_control_settings(4, settings, small_slice, 8, policy(shorter))
        .await
        .unwrap();
    assert_eq!(receipt.hosting_deadlines, shorter);
    assert_eq!(client.vm_bounds(), small_slice);
    assert_eq!(client.live_task_settings().current().revision, 5);
    assert_eq!(client.root_identity(), root);
    graceful(&mut owner).await;
}
