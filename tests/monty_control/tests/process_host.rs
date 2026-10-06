//! Actual subprocess/interpreter/allocator acceptance for the hosting candidate.
//! No Recipe, model response or host effect is simulated as successful.
use std::{
    collections::BTreeSet,
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};

use brassclaw_monty_host::{
    VmBounds, VmFailure,
    global::{GlobalBounds, Lifecycle},
    process::{
        GlobalProcess, PortAnswer, ProcessBoundary, ProcessFailure, ProcessLimits, RootBoot,
        WorkerCommand,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const SOURCE: &str = include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py");
fn boot(source: &str) -> RootBoot {
    RootBoot {
        source: source.to_owned(),
        checksum: Sha256::digest(source.as_bytes()).into(),
        aliases: [
            "await_next_task",
            "resolve_intent",
            "resolve_component_by_name",
            "compose_orchestrator",
            "run_program",
            "resolve_reply",
            "finish_task",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        bounds: GlobalBounds {
            values: VmBounds {
                max_source_bytes: 16384,
                max_compiled_source_bytes: 65536,
                max_feeds: 32,
                max_stdout_bytes: 1024,
                execution_slice: Duration::from_millis(5),
                max_value_depth: 16,
                max_value_nodes: 2048,
                max_value_bytes: 16384,
            },
            workers: 2,
            max_pending_calls: 8,
        },
        startup_timeout: Duration::from_secs(10),
    }
}
fn limits() -> ProcessLimits {
    ProcessLimits {
        hard_memory_bytes: 64 * 1024 * 1024,
        max_frame_bytes: 256 * 1024,
        response_timeout: Duration::from_secs(5),
    }
}
fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_global_worker"))
}
fn task() -> Value {
    json!({"task_token": "task-a", "conversation_id": "reborn-conv-opaque",
        "message_id": "message-a", "run_id": "run-a", "turn_id": "turn-a",
        "user_input": "'quoted' Ü {{vars.query}}", "history": []})
}

#[tokio::test]
async fn single_real_worker_waits_at_boot_rejects_replacement_and_preserves_admission() {
    let (mut process, ready) = GlobalProcess::start(worker(), boot(SOURCE), limits())
        .await
        .unwrap();
    let pid = process.process_id().unwrap();
    assert_eq!(ready.lifecycle, Lifecycle::Ready);
    assert_eq!(ready.work_waits.len(), 2);
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
    assert_eq!(replacement.snapshot.unwrap().work_waits, ready.work_waits);
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
    assert_eq!(rejected.snapshot.unwrap().work_waits, ready.work_waits);
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
        vec![json!("task-a"), json!("'quoted' Ü {{vars.query}}")]
    );
    assert!(kwargs.is_empty());
    let pending = process
        .exchange(WorkerCommand::Defer { key })
        .await
        .unwrap();
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
    let Some(ProcessBoundary::HostCall { name, args, .. }) = failed.boundary else {
        panic!("root must report its task failure through the actual finish boundary");
    };
    assert_eq!(name, "finish_task");
    assert_eq!(args[0], json!("task-a"));
    assert_eq!(args[1]["status"], json!("failed"));
    assert_eq!(process.process_id(), Some(pid));
    assert!(process.terminate().await.is_some());
    // Killing this worker proves neither a durable finish nor an external
    // effect's cancellation; the test deliberately leaves that port unresolved.
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
    let result = GlobalProcess::start(
        worker(),
        boot("huge = 'x' * 67108864\nwhile True:\n    pass"),
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
