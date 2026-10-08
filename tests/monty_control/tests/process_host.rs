//! Actual subprocess/interpreter/allocator acceptance for the hosting candidate.
//! No Recipe, model response or host effect is simulated as successful.
use std::{
    process::Stdio,
    time::{Duration, Instant},
};

use brassclaw_monty_host::{
    VmFailure,
    global::Lifecycle,
    process::{
        GlobalProcess, PortAnswer, ProcessBoundary, ProcessFailure, ProcessLimits, TaskSettings,
        WorkerCommand,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

mod support;
use support::{SOURCE, boot, limits, task, worker};

#[tokio::test]
async fn single_real_worker_waits_at_boot_rejects_replacement_and_preserves_admission() {
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    let pid = process.process_id().unwrap();
    assert_eq!(ready.lifecycle, Lifecycle::Ready);
    assert_eq!(ready.work_waits.len(), 2);
    let root = ready.root.unwrap();
    assert_eq!(
        root.source_checksum(),
        <[u8; 32]>::from(Sha256::digest(SOURCE.as_bytes()))
    );
    assert_eq!(root.workers(), 2);
    assert!(!root.vm_id().is_nil());

    let first_wait = ready.work_waits[0].1;
    // A second Boot cannot replace the already-running global VM.
    let replacement = process
        .exchange(WorkerCommand::Boot { boot: boot(SOURCE) })
        .await
        .unwrap_err();
    assert_eq!(
        replacement.kind,
        ProcessFailure::Vm(VmFailure::WrongBoundary)
    );
    let replacement = replacement.snapshot.unwrap();
    assert_eq!(replacement.root, Some(root));
    assert_eq!(replacement.work_waits, ready.work_waits);
    let mut invalid = task();
    invalid["lease_token"] = json!("private-claim-must-not-enter-python");
    let rejected = process
        .exchange(WorkerCommand::Admit {
            key: first_wait,
            task: invalid,
        })
        .await
        .unwrap_err();
    assert_eq!(rejected.kind, ProcessFailure::Vm(VmFailure::InvalidInputs));
    assert!(!format!("{rejected:?}").contains("private-claim"));
    assert!(matches!(
        rejected.command.as_deref(),
        Some(WorkerCommand::Admit { .. })
    ));
    let rejected = rejected.snapshot.unwrap();
    assert_eq!(rejected.root, Some(root));
    assert_eq!(rejected.work_waits, ready.work_waits);
    let mut oversized = task();
    oversized["user_input"] = json!("x".repeat(limits().max_frame_bytes));
    let overflow = process
        .exchange(WorkerCommand::Admit {
            key: first_wait,
            task: oversized,
        })
        .await
        .unwrap_err();
    assert_eq!(overflow.kind, ProcessFailure::ValueLimit);
    assert!(matches!(
        overflow.command.as_deref(),
        Some(WorkerCommand::Admit { .. })
    ));
    let mut oversized_boot = boot(SOURCE);
    oversized_boot.source = "x".repeat(limits().max_frame_bytes);
    let frame_overflow = process
        .exchange(WorkerCommand::Boot {
            boot: oversized_boot,
        })
        .await
        .unwrap_err();
    assert_eq!(frame_overflow.kind, ProcessFailure::FrameLimit);
    let mut nested = Value::Null;
    for _ in 0..128 {
        nested = json!([nested]);
    }
    let excessive_depth = process
        .exchange(WorkerCommand::Resolve {
            key: first_wait,
            answer: PortAnswer::Return { value: nested },
        })
        .await
        .unwrap_err();
    assert_eq!(excessive_depth.kind, ProcessFailure::ValueLimit);
    assert!(matches!(
        excessive_depth.command.as_deref(),
        Some(WorkerCommand::Resolve { .. })
    ));
    // Idle waiting has no lifetime/task timeout. The PID and same parked VM
    // continue to receive the later opaque admitted task.
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(process.process_id(), Some(pid));
    let admitted = process
        .exchange(WorkerCommand::Admit {
            key: first_wait,
            task: task(),
        })
        .await
        .unwrap();
    assert_eq!(admitted.root, Some(root));
    let task_handle = admitted.admitted_task.unwrap();
    let Some(ProcessBoundary::HostCall {
        key,
        name,
        args,
        kwargs,
    }) = admitted.boundary
    else {
        panic!("actual root must request intent resolution");
    };
    assert_eq!(name, "resolve_intent");
    assert_eq!(
        args,
        vec![
            json!(task_handle.to_string()),
            json!("'quoted' Ü {{vars.query}}")
        ]
    );
    assert!(kwargs.is_empty());
    let pending = process
        .exchange(WorkerCommand::Defer { key })
        .await
        .unwrap();
    assert_eq!(pending.root, Some(root));
    assert_eq!(pending.outstanding.len(), 2);
    assert_eq!(pending.work_waits.len(), 1);
    // Only a real port failure is supplied; no intent/model/Recipe success is
    // invented. Monty asks the durable finish port to record task failure.
    let failed = process
        .exchange(WorkerCommand::Resolve {
            key,
            answer: PortAnswer::DomainError {
                reason_kind: "intent_store_unavailable".into(),
            },
        })
        .await
        .unwrap();
    assert_eq!(failed.root, Some(root));
    let Some(ProcessBoundary::HostCall { name, args, .. }) = failed.boundary else {
        panic!("root must report its task failure through the actual finish boundary");
    };
    assert_eq!(name, "finish_task");
    assert_eq!(args[0], json!(task_handle.to_string()));
    assert_eq!(args[1]["status"], json!("failed"));
    assert_eq!(process.process_id(), Some(pid));
    assert!(process.terminate().await.is_some());
    // Killing this worker proves neither a durable finish nor an external
    // effect's cancellation; the test deliberately leaves that port unresolved.
}

#[tokio::test]
async fn worker_rejects_wrong_root_checksum_before_issuing_execution_identity() {
    let mut requested = boot(SOURCE);
    requested.checksum[0] ^= 1;
    let failure = match GlobalProcess::start(worker(), requested, limits()).await {
        Ok(_) => panic!("wrong root source integrity must prevent startup"),
        Err(failure) => failure,
    };
    assert_eq!(failure.kind, ProcessFailure::Vm(VmFailure::Integrity));
    assert!(failure.snapshot.unwrap().root.is_none());
    assert!(failure.exit_status.is_some());
}

#[tokio::test]
async fn actual_idle_worker_shutdown_is_explicit_and_reaped() {
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    process
        .exchange(WorkerCommand::BeginShutdown)
        .await
        .unwrap();
    let first = process
        .exchange(WorkerCommand::CloseWorker {
            key: ready.work_waits[0].1,
        })
        .await
        .unwrap();
    assert_eq!(first.lifecycle, Lifecycle::Stopping);
    let last = process
        .exchange(WorkerCommand::CloseWorker {
            key: ready.work_waits[1].1,
        })
        .await
        .unwrap();
    assert_eq!(last.lifecycle, Lifecycle::Stopped);
    assert!(last.outstanding.is_empty());
    assert!(matches!(last.boundary, Some(ProcessBoundary::Stopped)));
    let status = process
        .wait_stopped()
        .await
        .expect("worker exit must be acknowledged");
    assert!(status.success());
}

#[tokio::test]
async fn parent_deadline_contains_actual_busy_startup_and_keeps_command_evidence() {
    let started = Instant::now();
    // The interpreter's startup budget is ten seconds. The parent independently
    // interrupts and reaps it much earlier; this does not depend on VM polling.
    let result = GlobalProcess::start(
        worker(),
        boot("while True:\n    pass"),
        ProcessLimits {
            response_timeout: Duration::from_millis(100),
            ..limits()
        },
    )
    .await;
    let Err(error) = result else {
        panic!("busy root cannot become Ready");
    };
    assert_eq!(error.kind, ProcessFailure::Deadline);
    assert!(matches!(
        error.command.as_deref(),
        Some(WorkerCommand::Boot { .. })
    ));
    assert!(error.exit_status.is_some());
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn real_native_allocation_stops_only_worker_and_preserves_failed_boot() {
    // Deliberately exercise the independent physical backstop without a soft
    // preflight. Instance ServiceOwner rejects this probe-only configuration.
    let mut physical_probe = boot("huge = 'x' * 67108864\nwhile True:\n    pass");
    physical_probe.heap_settings = None;
    let result = GlobalProcess::start(
        worker(),
        physical_probe,
        ProcessLimits {
            hard_memory_bytes: 8 * 1024 * 1024,
            ..limits()
        },
    )
    .await;
    let Err(error) = result else {
        panic!("worker allocation must hit the finite physical ceiling");
    };
    assert_eq!(error.kind, ProcessFailure::Transport);
    assert_eq!(
        error.exit_status.unwrap().code(),
        Some(monty_types::OOM_EXIT_CODE)
    );
    assert!(matches!(
        error.command.as_deref(),
        Some(WorkerCommand::Boot { .. })
    ));
    // The same parent remains usable after the allocator's fatal worker exit.
    let (mut replacement, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    assert_eq!(ready.lifecycle, Lifecycle::Ready);
    assert!(replacement.terminate().await.is_some());
}

#[tokio::test]
async fn dropping_busy_exchange_fences_worker_and_retains_interrupted_admission() {
    let source = "import asyncio\nasync def worker(i):\n    task = await host.await_next_task(i)\n    while True:\n        pass\nasync def main():\n    await asyncio.gather(worker(0), worker(1))\nasyncio.run(main())";
    let mut root = boot(source);
    root.bounds.values.execution_slice = Duration::from_secs(60);
    let (mut process, ready) = GlobalProcess::start(worker(), root, limits())
        .await
        .unwrap();
    let result = tokio::time::timeout(
        Duration::from_millis(20),
        process.exchange(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        }),
    )
    .await;
    assert!(
        result.is_err(),
        "actual busy root must not answer before caller cancellation"
    );
    assert!(matches!(
        process.take_interrupted_command().as_deref(),
        Some(WorkerCommand::Admit { .. })
    ));
    let fenced = process
        .exchange(WorkerCommand::BeginShutdown)
        .await
        .unwrap_err();
    assert_eq!(fenced.kind, ProcessFailure::Terminal);
    assert!(process.terminate().await.is_some());
}

#[tokio::test]
async fn real_worker_rejects_malformed_frames_without_private_diagnostics() {
    let mut unknown = json!({"protocol": 1, "sequence": 1,
        "command": {"operation": "boot", "boot": boot(SOURCE)}});
    unknown["lease_token"] = json!("private-claim-never-log");
    let serialized = serde_json::to_vec(&unknown).unwrap();
    for (length, body) in [
        (0u32, Vec::new()),
        (limits().max_frame_bytes as u32 + 1, Vec::new()),
        (serialized.len() as u32, serialized),
    ] {
        let mut child = tokio::process::Command::new(worker())
            .arg(limits().hard_memory_bytes.to_string())
            .arg(limits().max_frame_bytes.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&length.to_be_bytes()).await.unwrap();
        stdin.write_all(&body).await.unwrap();
        drop(stdin);
        let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
            .await
            .unwrap()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("invalid worker request"));
        assert!(!diagnostic.contains("private-claim"));
    }
}

async fn recipe_command(
    process: &mut GlobalProcess,
    command: brassclaw_monty_host::process::RecipeCommand,
) -> brassclaw_monty_host::process::ProcessSnapshot {
    process
        .exchange(WorkerCommand::Recipe { command })
        .await
        .unwrap()
}

#[tokio::test]
async fn task_cancellation_fences_children_and_settles_the_actual_root_future() {
    use brassclaw_monty_host::process::{RecipeCommand, RecipeEvent, TaskHandle};
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    let admitted = process
        .exchange(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .await
        .unwrap();
    let task = admitted.admitted_task.unwrap();
    let Some(ProcessBoundary::HostCall { key, name, .. }) = admitted.boundary else {
        panic!("admission must reach the actual intent port")
    };
    assert_eq!(name, "resolve_intent");
    process
        .exchange(WorkerCommand::Defer { key })
        .await
        .unwrap();
    let context = open_context(&mut process, task, None).await;
    let unknown: TaskHandle =
        serde_json::from_value(json!("00000000-0000-0000-0000-000000000000")).unwrap();
    let denied = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::CancelTask { task: unknown },
        })
        .await
        .unwrap_err();
    assert_eq!(
        denied.kind,
        ProcessFailure::Vm(VmFailure::ForeignContinuation)
    );
    let cancelled = recipe_command(&mut process, RecipeCommand::CancelTask { task }).await;
    assert!(matches!(cancelled.recipe,
        Some(RecipeEvent::CancellationRequested { task: actual }) if actual == task));
    assert_eq!(cancelled.lifecycle, Lifecycle::Ready);
    assert_eq!(cancelled.outstanding.len(), 2);
    assert_eq!(cancelled.task_accounting.len(), 1);
    let refused = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task,
                parent: Some(context),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ProcessFailure::Vm(VmFailure::Terminal));
    let rejected = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::CloseTask { task },
        })
        .await
        .unwrap_err();
    assert_eq!(rejected.kind, ProcessFailure::Vm(VmFailure::WrongBoundary));
    let failed = process
        .exchange(WorkerCommand::Resolve {
            key,
            answer: PortAnswer::DomainError {
                reason_kind: "actual_intent_host_error".into(),
            },
        })
        .await
        .unwrap();
    let Some(ProcessBoundary::HostCall { name, args, .. }) = failed.boundary else {
        panic!("the cancelled worker must reach its task failure handler")
    };
    assert_eq!(name, "finish_task");
    assert_eq!(args[1]["reason_kind"], json!("task_cancelled"));
    assert_eq!(failed.lifecycle, Lifecycle::Ready);
    assert_eq!(failed.work_waits, vec![ready.work_waits[1]]);
    assert_eq!(failed.withheld_answers.len(), 1);
    assert_eq!(failed.withheld_answers[0].continuation, key);
    assert!(matches!(&failed.withheld_answers[0].answer,
        brassclaw_monty_host::HostAnswer::Raise(error)
        if error.to_string().contains("actual_intent_host_error")));
    // Neither the finish port nor external effect completion is acknowledged.
    assert!(process.terminate().await.is_some());
}
async fn open_context(
    process: &mut GlobalProcess,
    task: brassclaw_monty_host::process::TaskHandle,
    parent: Option<brassclaw_monty_host::process::RecipeContextId>,
) -> brassclaw_monty_host::process::RecipeContextId {
    use brassclaw_monty_host::process::{RecipeCommand, RecipeEvent};
    match recipe_command(process, RecipeCommand::Open { task, parent })
        .await
        .recipe
        .unwrap()
    {
        RecipeEvent::Opened { context, .. } => context,
        _ => panic!("actual context opening required"),
    }
}
fn selected(source: &str, aliases: &[&str]) -> brassclaw_monty_host::process::SelectedPython {
    brassclaw_monty_host::process::SelectedPython {
        source: source.into(),
        checksum: Sha256::digest(source.as_bytes()).into(),
        aliases: aliases.iter().map(|s| (*s).into()).collect(),
    }
}
async fn start_context(
    process: &mut GlobalProcess,
    context: brassclaw_monty_host::process::RecipeContextId,
    source: &str,
    inputs: Value,
    aliases: &[&str],
) -> brassclaw_monty_host::process::RecipeBoundary {
    use brassclaw_monty_host::process::{RecipeBoundary, RecipeCommand, RecipeEvent};
    let mut snapshot = recipe_command(
        process,
        RecipeCommand::Start {
            context,
            selected: selected(source, aliases),
            inputs,
        },
    )
    .await;
    loop {
        match snapshot.recipe.unwrap() {
            RecipeEvent::Progress {
                boundary: RecipeBoundary::ControlYield { key },
                ..
            } => {
                snapshot =
                    recipe_command(process, RecipeCommand::ResumeControl { context, key }).await;
            }
            RecipeEvent::Progress { boundary, .. } => return boundary,
            _ => panic!("actual interpreter progress required"),
        }
    }
}

#[tokio::test]
async fn heap_growth_resizes_the_real_backstop_without_restarting_or_resetting_state() {
    use brassclaw_monty_host::heap::HeapSettings;
    use brassclaw_monty_host::process::{RecipeBoundary, RecipeCommand};
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    let pid = process.process_id();
    let expanded = HeapSettings {
        revision: 2,
        max_vm_bytes: 96 * 1024 * 1024,
    };
    let grown = process
        .exchange(WorkerCommand::UpdateHeap {
            expected_revision: 1,
            settings: expanded,
            automatic: true,
        })
        .await
        .unwrap();
    assert_eq!(grown.heap.effective, Some(expanded));
    assert_eq!(grown.root, ready.root);
    assert_eq!(grown.effective_task_settings, ready.effective_task_settings);
    assert_eq!(process.process_id(), pid);
    let admitted = process
        .exchange(WorkerCommand::Admit {
            key: grown.work_waits[0].1,
            task: task(),
        })
        .await
        .unwrap();
    let handle = admitted.admitted_task.unwrap();
    let Some(ProcessBoundary::HostCall { key, .. }) = admitted.boundary else {
        panic!("actual intent boundary required")
    };
    process
        .exchange(WorkerCommand::Defer { key })
        .await
        .unwrap();
    let context = open_context(&mut process, handle, None).await;
    assert!(matches!(
        start_context(&mut process, context, "held = 'x' * 73400320\nresult = len(held)", json!({}), &[]).await,
        RecipeBoundary::Complete { value } if value == json!(73400320)
    ));
    let retained = process.exchange(WorkerCommand::Inspect).await.unwrap();
    assert!(
        retained.vm_live_bytes > limits().hard_memory_bytes,
        "real retained interpreter storage must exceed the original physical cap"
    );
    for (bytes, failure) in [
        (16 * 1024 * 1024, VmFailure::UnsafeHeapReduction),
        (usize::MAX, VmFailure::InvalidBounds),
    ] {
        let rejected = process
            .exchange(WorkerCommand::UpdateHeap {
                expected_revision: 2,
                settings: HeapSettings {
                    revision: 3,
                    max_vm_bytes: bytes,
                },
                automatic: false,
            })
            .await
            .unwrap_err();
        assert_eq!(rejected.kind, ProcessFailure::Vm(failure));
        assert_eq!(rejected.snapshot.unwrap().heap, retained.heap);
        assert_eq!(process.process_id(), pid);
    }
    assert!(
        matches!(start_context(&mut process, context, "result = len(held)", json!({}), &[]).await,
        RecipeBoundary::Complete { value } if value == json!(73400320))
    );
    recipe_command(&mut process, RecipeCommand::CancelContext { context }).await;
    let shrunk = process
        .exchange(WorkerCommand::UpdateHeap {
            expected_revision: 2,
            settings: HeapSettings {
                revision: 3,
                max_vm_bytes: 16 * 1024 * 1024,
            },
            automatic: false,
        })
        .await
        .unwrap();
    assert_eq!(shrunk.heap.effective.unwrap().revision, 3);
    assert!(shrunk.vm_live_bytes < 16 * 1024 * 1024);
    assert_eq!(shrunk.root, ready.root);
    assert_eq!(process.process_id(), pid);
    // The unresolved root call is contained, not reported as completed.
    assert!(process.terminate().await.is_some());
}

#[tokio::test]
async fn actual_shared_vm_bytes_retain_child_state_and_refund_only_released_context() {
    use brassclaw_monty_host::heap::HeapSettings;
    use brassclaw_monty_host::process::{RecipeBoundary, RecipeCommand};
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    assert!(
        ready.vm_live_bytes > 0,
        "compiled root must be charged at boot"
    );
    let admitted = process
        .exchange(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .await
        .unwrap();
    let task_handle = admitted.admitted_task.unwrap();
    let Some(ProcessBoundary::HostCall { key, .. }) = admitted.boundary else {
        panic!("actual root intent boundary required")
    };
    process
        .exchange(WorkerCommand::Defer { key })
        .await
        .unwrap();
    let parent = open_context(&mut process, task_handle, None).await;
    assert!(matches!(start_context(&mut process, parent,
        "held = 'x' * 524288\nresult = len(held)", json!({}), &[]).await,
        RecipeBoundary::Complete { value } if value == json!(524288)));
    let parent_usage = process
        .exchange(WorkerCommand::Inspect)
        .await
        .unwrap()
        .vm_live_bytes;
    assert!(parent_usage >= ready.vm_live_bytes + 524288);
    let child = open_context(&mut process, task_handle, Some(parent)).await;
    assert!(matches!(start_context(&mut process, child,
        "held = 'y' * 524288\nresult = len(held)", json!({}), &[]).await,
        RecipeBoundary::Complete { value } if value == json!(524288)));
    let both = process.exchange(WorkerCommand::Inspect).await.unwrap();
    assert!(both.vm_live_bytes >= parent_usage + 524288);
    let idle = process.exchange(WorkerCommand::Inspect).await.unwrap();
    assert_eq!(
        idle.vm_live_bytes, both.vm_live_bytes,
        "transport receipts must not inflate the VM account"
    );
    let reduced = HeapSettings {
        revision: 2,
        max_vm_bytes: 1024 * 1024,
    };
    let unsafe_manual = process
        .exchange(WorkerCommand::UpdateHeap {
            expected_revision: 1,
            settings: reduced,
            automatic: false,
        })
        .await
        .unwrap_err();
    assert_eq!(
        unsafe_manual.kind,
        ProcessFailure::Vm(VmFailure::UnsafeHeapReduction)
    );
    assert_eq!(unsafe_manual.snapshot.unwrap().heap, both.heap);
    let pending = process
        .exchange(WorkerCommand::UpdateHeap {
            expected_revision: 1,
            settings: reduced,
            automatic: true,
        })
        .await
        .unwrap();
    assert_eq!(pending.heap.desired, Some(reduced));
    assert_eq!(pending.heap.effective, both.heap.effective);
    assert!(pending.heap.pending_reduction);
    let mut another = task();
    another["run_id"] = json!("heap-pending-unadmitted-run");
    let no_capacity = process
        .exchange(WorkerCommand::Admit {
            key: pending.work_waits[0].1,
            task: another,
        })
        .await
        .unwrap_err();
    assert_eq!(
        no_capacity.kind,
        ProcessFailure::Vm(VmFailure::HeapBackpressure)
    );
    assert_eq!(no_capacity.snapshot.unwrap().task_accounting.len(), 1);
    let stale = process
        .exchange(WorkerCommand::UpdateHeap {
            expected_revision: 1,
            settings: HeapSettings {
                revision: 3,
                max_vm_bytes: 2 * 1024 * 1024,
            },
            automatic: false,
        })
        .await
        .unwrap_err();
    assert_eq!(
        stale.kind,
        ProcessFailure::Vm(VmFailure::SettingsRevisionConflict)
    );
    recipe_command(
        &mut process,
        RecipeCommand::CancelContext { context: child },
    )
    .await;
    let remaining = process.exchange(WorkerCommand::Inspect).await.unwrap();
    assert!(remaining.vm_live_bytes + 524288 <= both.vm_live_bytes);
    assert!(
        remaining.vm_live_bytes >= ready.vm_live_bytes + 524288,
        "releasing a child must not refund its parent's retained objects"
    );
    assert_eq!(remaining.heap.effective, Some(reduced));
    assert!(!remaining.heap.pending_reduction);
    let too_large = open_context(&mut process, task_handle, Some(parent)).await;
    let allocation = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::Start {
                context: too_large,
                selected: selected("result = 'z' * 2097152", &[]),
                inputs: json!({}),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(allocation.kind, ProcessFailure::Vm(VmFailure::Python));
    assert!(
        allocation
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("memory limit exceeded")
    );
    assert_eq!(
        allocation.snapshot.unwrap().lifecycle,
        Lifecycle::Ready,
        "preflight rejection must not corrupt the unrelated root or retained parent"
    );
    recipe_command(
        &mut process,
        RecipeCommand::CancelContext { context: too_large },
    )
    .await;
    assert!(matches!(start_context(&mut process, parent,
        "result = len(held)", json!({}), &[]).await,
        RecipeBoundary::Complete { value } if value == json!(524288)));
    // Actual root host call remains unresolved; terminating the worker is only
    // containment, not a fabricated effect or task-completion acknowledgement.
    assert!(process.terminate().await.is_some());
}

#[tokio::test]
async fn worker_recipe_state_child_handoff_and_task_cancellation_keep_root_alive() {
    use brassclaw_monty_host::process::{RecipeBoundary, RecipeCommand, RecipeEvent};
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    let pid = process.process_id();
    let first = process
        .exchange(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .await
        .unwrap();
    let a = first.admitted_task.unwrap();
    let root_compute = first.task_accounting[0].compute_time.unwrap();
    assert!(
        root_compute > Duration::ZERO,
        "root execution is charged before its first port"
    );
    let Some(ProcessBoundary::HostCall { key, .. }) = first.boundary else {
        panic!("intent boundary required")
    };
    let waiting = process
        .exchange(WorkerCommand::Defer { key })
        .await
        .unwrap();
    let parked_compute = waiting.task_accounting[0].compute_time.unwrap();
    assert!(parked_compute >= root_compute);
    let mut second_input = task();
    second_input["run_id"] = json!("run-b");
    second_input["conversation_id"] = json!("another-opaque-conversation");
    let second = process
        .exchange(WorkerCommand::Admit {
            key: waiting.work_waits[0].1,
            task: second_input,
        })
        .await
        .unwrap();
    let b = second.admitted_task.unwrap();
    assert_ne!(a, b);
    assert_eq!(
        second
            .task_accounting
            .iter()
            .find(|entry| entry.task == a)
            .unwrap()
            .compute_time,
        Some(parked_compute)
    );
    assert!(
        second
            .task_accounting
            .iter()
            .find(|entry| entry.task == b)
            .unwrap()
            .compute_time
            .unwrap()
            > Duration::ZERO
    );
    let context_a = open_context(&mut process, a, None).await;
    let context_b = open_context(&mut process, b, None).await;
    let foreign_parent = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task: b,
                parent: Some(context_a),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        foreign_parent.kind,
        ProcessFailure::Vm(VmFailure::ForeignContinuation)
    );
    let hostile = "'quoted' Ü {{vars.query}}\nresult = 'source-injection'";
    let result = start_context(
        &mut process,
        context_a,
        "saved = inputs['value']\nresult = saved",
        json!({"value": hostile}),
        &[],
    )
    .await;
    assert!(matches!(result, RecipeBoundary::Complete { value } if value == json!(hostile)));
    let result = start_context(
        &mut process,
        context_b,
        "saved = 'task-b-value'\nresult = saved",
        json!({}),
        &[],
    )
    .await;
    assert!(matches!(result, RecipeBoundary::Complete { value } if value == json!("task-b-value")));
    let RecipeBoundary::HostCall {
        key: parent_key, ..
    } = start_context(
        &mut process,
        context_a,
        "child_value = host.child()\nresult = [saved, child_value]",
        json!({}),
        &["child"],
    )
    .await
    else {
        panic!("real parent handoff required")
    };
    let child = open_context(&mut process, a, Some(context_a)).await;
    let RecipeBoundary::Complete { value } = start_context(
        &mut process,
        child,
        "result = inputs['number'] * 7",
        json!({"number": 6}),
        &[],
    )
    .await
    else {
        panic!("real child computation required")
    };
    let foreign = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::ResumeHost {
                context: context_b,
                key: parent_key,
                answer: PortAnswer::Return {
                    value: value.clone(),
                },
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        foreign.kind,
        ProcessFailure::Vm(VmFailure::ForeignContinuation)
    );
    let parent = recipe_command(
        &mut process,
        RecipeCommand::ResumeHost {
            context: context_a,
            key: parent_key,
            answer: PortAnswer::Return { value },
        },
    )
    .await;
    assert!(
        matches!(parent.recipe, Some(RecipeEvent::Progress { boundary: RecipeBoundary::Complete { value }, .. }) if value == json!([hostile, 42]))
    );
    let RecipeBoundary::HostCall { key: child_key, .. } = start_context(
        &mut process,
        child,
        "result = host.pending()",
        json!({}),
        &["pending"],
    )
    .await
    else {
        panic!("real pending child required")
    };
    let released = recipe_command(
        &mut process,
        RecipeCommand::CancelContext { context: context_a },
    )
    .await;
    let Some(RecipeEvent::Released { contexts, .. }) = released.recipe else {
        panic!("cancellation receipt required")
    };
    assert_eq!(contexts.len(), 2);
    assert!(
        contexts
            .iter()
            .any(|entry| entry.context == child && entry.pending_host == Some(child_key))
    );
    assert_eq!(process.process_id(), pid);
    assert_eq!(released.lifecycle, Lifecycle::Ready);
    let reopen = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task: a,
                parent: None,
            },
        })
        .await
        .unwrap_err();
    assert_eq!(reopen.kind, ProcessFailure::Vm(VmFailure::WrongBoundary));
    let late = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::ResumeHost {
                context: child,
                key: child_key,
                answer: PortAnswer::Return {
                    value: json!("late"),
                },
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        late.kind,
        ProcessFailure::Vm(VmFailure::ForeignContinuation)
    );
    assert!(late.command.is_some());
    let result = start_context(&mut process, context_b, "result = 40 + 2", json!({}), &[]).await;
    assert!(matches!(result, RecipeBoundary::Complete { value } if value == json!(42)));
    // A real running child yields at the bytecode checkpoint. Releasing it
    // acknowledges only VM state and leaves the root's real ports unchanged.
    let busy = recipe_command(
        &mut process,
        RecipeCommand::Start {
            context: context_b,
            selected: selected("while True:\n    pass", &[]),
            inputs: json!({}),
        },
    )
    .await;
    assert!(matches!(
        busy.recipe,
        Some(RecipeEvent::Progress {
            boundary: RecipeBoundary::ControlYield { .. },
            ..
        })
    ));
    let stopped = recipe_command(
        &mut process,
        RecipeCommand::CancelContext { context: context_b },
    )
    .await;
    assert_eq!(stopped.lifecycle, Lifecycle::Ready);
    assert_eq!(process.process_id(), pid);
    for task in [a, b] {
        let premature = process
            .exchange(WorkerCommand::Recipe {
                command: RecipeCommand::CloseTask { task },
            })
            .await
            .unwrap_err();
        assert_eq!(premature.kind, ProcessFailure::Vm(VmFailure::WrongBoundary));
        let snapshot = premature.snapshot.unwrap();
        assert_eq!(snapshot.task_accounting.len(), 2);
        assert!(
            snapshot
                .task_accounting
                .iter()
                .find(|entry| entry.task == a)
                .unwrap()
                .compute_time
                .unwrap()
                > root_compute
        );
    }
    assert!(process.terminate().await.is_some());
    // The root's real intent ports remain unresolved. Local context release
    // proves no durable finish or external effect settlement.
}

#[tokio::test]
async fn worker_live_settings_and_uncatchable_child_abort_preserve_other_tasks() {
    use brassclaw_monty_host::process::{RecipeBoundary, RecipeCommand};
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    let admitted = process
        .exchange(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .await
        .unwrap();
    let task = admitted.admitted_task.unwrap();
    let context = open_context(&mut process, task, None).await;
    let RecipeBoundary::HostCall { key, .. } = start_context(
        &mut process,
        context,
        "try:\n    result = host.pending()\nexcept Exception:\n    result = 'incorrectly-caught'",
        json!({}),
        &["pending"],
    )
    .await
    else {
        panic!("actual host boundary required")
    };
    let updated = recipe_command(
        &mut process,
        RecipeCommand::UpdateSettings {
            expected_revision: 1,
            settings: TaskSettings {
                revision: 2,
                max_compute_time: Duration::from_secs(300),
                token_budgets_enabled: true,
            },
        },
    )
    .await;
    assert_eq!(updated.effective_task_settings.unwrap().revision, 2);
    let used = updated.task_accounting[0].compute_time.unwrap();
    assert!(used > Duration::ZERO);
    assert_eq!(updated.task_accounting[0].effective_revision, 2);
    let stale = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::UpdateSettings {
                expected_revision: 1,
                settings: TaskSettings {
                    revision: 3,
                    max_compute_time: Duration::from_secs(400),
                    token_budgets_enabled: false,
                },
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        stale.kind,
        ProcessFailure::Vm(VmFailure::SettingsRevisionConflict)
    );
    assert_eq!(
        stale
            .snapshot
            .unwrap()
            .effective_task_settings
            .unwrap()
            .revision,
        2
    );
    let denied = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::ResumeHost {
                context,
                key,
                answer: PortAnswer::TerminalError {
                    reason_kind: "policy_denied".into(),
                },
            },
        })
        .await
        .unwrap_err();
    assert_eq!(denied.kind, ProcessFailure::Vm(VmFailure::Python));
    let snapshot = denied.snapshot.unwrap();
    assert_eq!(snapshot.lifecycle, Lifecycle::Ready);
    assert!(snapshot.task_accounting[0].compute_time.unwrap() >= used);
    let root_wait = snapshot.work_waits[0].1;
    let root_abort = process
        .exchange(WorkerCommand::Resolve {
            key: root_wait,
            answer: PortAnswer::TerminalError {
                reason_kind: "policy_denied".into(),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(
        root_abort.kind,
        ProcessFailure::Vm(VmFailure::InvalidHostArguments)
    );
    process
        .exchange(WorkerCommand::BeginShutdown)
        .await
        .unwrap();
    let premature = process
        .exchange(WorkerCommand::CloseWorker { key: root_wait })
        .await
        .unwrap_err();
    assert_eq!(premature.kind, ProcessFailure::Vm(VmFailure::WrongBoundary));
    let active_close = process
        .exchange(WorkerCommand::Recipe {
            command: RecipeCommand::CloseTask { task },
        })
        .await
        .unwrap_err();
    assert_eq!(
        active_close.kind,
        ProcessFailure::Vm(VmFailure::WrongBoundary)
    );
    assert_eq!(active_close.snapshot.unwrap().task_accounting.len(), 1);
    assert!(process.terminate().await.is_some());
}
