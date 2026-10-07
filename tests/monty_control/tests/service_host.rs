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
}
fn failure(reason: &str) -> PortFailure {
    PortFailure::new(reason).unwrap()
}
impl TaskPorts for FilePorts {
    fn call(
        self: Arc<Self>,
        _task: TaskHandle,
        _transport: TransportClient,
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
                    self.started.notify_one();
                    if self.pause_read {
                        self.release.notified().await;
                    }
                    let value = tokio::fs::read_to_string(&self.input)
                        .await
                        .map_err(|_| failure("file_read_failed"))?;
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
        4,
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
                max_compute_time: Duration::from_secs(300),
                token_budgets_enabled: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(first.effective_settings.revision, 2);
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
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.effective_settings.revision, 3);
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
