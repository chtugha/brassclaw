//! Real worker actor acceptance. Dropped callers never own the IPC future.
use brassclaw_monty_host::{
    global::Lifecycle,
    process::{
        ProcessBoundary, ProcessFailure, RecipeCommand, RecipeEvent, SelectedPython, TaskSettings,
        WorkerCommand,
    },
    transport_actor::{ActorFailure, ActorLimits, StopKind, TransportClient, TransportOwner},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
    time::Duration,
};
mod support;
use support::{SOURCE, boot, limits, task, worker};

fn actor_limits() -> ActorLimits {
    ActorLimits {
        max_unclaimed: 4,
        max_reserved_frame_bytes: limits().max_frame_bytes * 8,
        max_control_unclaimed: 4,
        max_control_reserved_frame_bytes: limits().max_frame_bytes * 8,
    }
}
async fn exchange(
    client: &TransportClient,
    command: WorkerCommand,
) -> brassclaw_monty_host::process::ProcessSnapshot {
    let ticket = client.try_submit(command).unwrap();
    tokio::time::timeout(Duration::from_secs(15), ticket.wait())
        .await
        .unwrap()
        .unwrap()
        .outcome
        .unwrap()
}

#[tokio::test]
async fn live_frame_policy_preserves_old_receipts_and_reclaims_capacity_after_last_refund() {
    use brassclaw_monty_host::heap::FrameUpdate;
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
            .await
            .unwrap();
    let client = owner.client();
    let clone = client.clone();
    let initial = ready.vm_bounds.unwrap();
    let update = |expected, configured, required| WorkerCommand::Recipe {
        command: RecipeCommand::UpdateRuntimeSettings {
            expected_revision: expected,
            settings: TaskSettings {
                revision: expected + 1,
                max_compute_time: Duration::from_secs(600),
                token_budgets_enabled: false,
            },
            values: initial,
            max_recipe_contexts: None,
            adapter_reserve_bytes: None,
            heap_update: None,
            frame_update: Some(FrameUpdate {
                configured_frame_bytes: configured,
                required_capacity_bytes: required,
            }),
        },
    };
    let grown = exchange(&client, update(1, 512 * 1024, 512 * 1024)).await;
    assert_eq!(grown.root, ready.root);
    assert_eq!(clone.capacity().unwrap().configured_frame_bytes, 512 * 1024);
    let mut old_ids = Vec::new();
    for _ in 0..2 {
        let ticket = clone.try_submit(WorkerCommand::Inspect).unwrap();
        old_ids.push(ticket.id);
        drop(ticket);
    }
    for id in &old_ids {
        client.completions().ready(*id).await.unwrap();
    }
    let shrunk = exchange(&client, update(2, 65536, 512 * 1024)).await;
    assert_eq!(shrunk.frame.configured_frame_bytes, 65536);
    assert_eq!(shrunk.frame.effective_capacity_bytes, 512 * 1024);
    assert_eq!(client.frame_status(), shrunk.frame);
    assert_eq!(clone.capacity().unwrap().configured_frame_bytes, 65536);
    assert_eq!(
        clone.capacity().unwrap().max_retained_frame_bytes,
        512 * 1024
    );
    let new = clone
        .try_submit(WorkerCommand::Inspect)
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(new.max_frame_bytes, 65536);
    assert_eq!(new.outcome.unwrap().root, ready.root);
    let denied = client
        .try_submit(WorkerCommand::ReconcileFrameCapacity {
            expected_settings_revision: 3,
            configured_frame_bytes: 65536,
            required_capacity_bytes: 65536,
        })
        .err()
        .expect("old credited responses still need their capacity");
    assert_eq!(denied.kind, ActorFailure::InvalidLimits);
    assert!(matches!(
        *denied.command,
        WorkerCommand::ReconcileFrameCapacity { .. }
    ));
    let first = client.completions().try_take(old_ids[0]).unwrap();
    assert_eq!(first.max_frame_bytes, 512 * 1024);
    assert_eq!(first.outcome.unwrap().root, ready.root);
    assert_eq!(client.frame_status().effective_capacity_bytes, 512 * 1024);
    let mut observed = client.live_worker_memory();
    let before = *observed.borrow_and_update();
    assert_eq!(before.frame.effective_capacity_bytes, 512 * 1024);
    let last = client.completions().try_take(old_ids[1]).unwrap();
    assert_eq!(last.max_frame_bytes, 512 * 1024);
    assert_eq!(last.outcome.unwrap().root, ready.root);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if observed.borrow_and_update().frame.effective_capacity_bytes == 65536 {
                break;
            }
            observed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(client.capacity().unwrap().max_retained_frame_bytes, 0);
    assert_eq!(client.capacity().unwrap().control_reserved_bytes, 0);
    assert_eq!(
        client.allocator_status().non_vm_reserve_bytes,
        2 * 65536 + grown.allocator.adapter_reserve_bytes
    );
    let state = exchange(&clone, WorkerCommand::Inspect).await;
    assert_eq!(state.root, ready.root);
    assert_eq!(state.heap, ready.heap);
    assert_eq!(state.effective_task_settings.unwrap().revision, 3);
    let denied = clone
        .try_submit_with_frame_limit(WorkerCommand::Inspect, 512 * 1024)
        .err()
        .expect("new exchanges use current policy");
    assert_eq!(denied.kind, ActorFailure::InvalidLimits);
    owner.request_termination();
    assert_eq!(owner.join().await.unwrap().kind, StopKind::Requested);
}

#[tokio::test]
async fn retained_frame_credits_survive_policy_changes_and_abandoned_tickets() {
    use brassclaw_monty_host::transport_actor::HostingDeadlines;
    let initial_limits = actor_limits();
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(SOURCE), limits(), initial_limits)
            .await
            .unwrap();
    let client = owner.client();
    let initial_deadline = client.hosting_deadlines().unwrap().response_timeout;
    let tighter = 8192;
    let old = client
        .try_submit_with_frame_limit(WorkerCommand::Inspect, tighter)
        .unwrap();
    let id = old.id;
    drop(old);
    let ordinary_frame = client.try_submit(WorkerCommand::Inspect).unwrap();
    let other_frame = client.clone().try_submit(WorkerCommand::Inspect).unwrap();
    let capacity = client.capacity().unwrap();
    assert_eq!(capacity.configured_frame_bytes, limits().max_frame_bytes);
    assert_eq!(capacity.max_retained_frame_bytes, limits().max_frame_bytes);
    assert_eq!(capacity.required_frame_capacity(), limits().max_frame_bytes);
    assert_eq!(capacity.control_requests, 3);
    assert!(capacity.control_reserved_bytes > limits().max_frame_bytes * 2 + tighter);
    assert!(capacity.control_reserved_bytes < limits().max_frame_bytes * 3);
    for invalid in [0, limits().max_frame_bytes + 1] {
        assert_eq!(
            client
                .try_submit_with_frame_limit(WorkerCommand::Inspect, invalid)
                .err()
                .expect("invalid frame must be rejected before acceptance")
                .kind,
            ActorFailure::InvalidLimits
        );
        assert_eq!(client.capacity().unwrap(), capacity);
    }
    client
        .publish_hosting_policy(
            Some(ActorLimits {
                max_control_reserved_frame_bytes: limits().max_frame_bytes * 2,
                ..initial_limits
            }),
            Some(HostingDeadlines {
                startup_timeout: Duration::from_secs(2),
                response_timeout: Duration::from_secs(8),
            }),
        )
        .unwrap();
    assert!(client.capacity().unwrap().control_over_capacity());
    assert_eq!(
        client
            .try_submit(WorkerCommand::Inspect)
            .err()
            .expect("retained capacity debt must reject a new exchange")
            .kind,
        ActorFailure::Backpressure
    );
    let retained = client.completions().wait(id).await.unwrap();
    assert_eq!(retained.max_frame_bytes, tighter);
    assert_eq!(retained.response_timeout, initial_deadline);
    assert!(matches!(
        retained.original_command().unwrap(),
        WorkerCommand::Inspect
    ));
    assert_eq!(retained.outcome.unwrap().root, ready.root);
    assert!(client.capacity().unwrap().control_over_capacity());
    let original = ordinary_frame.wait().await.unwrap();
    assert_eq!(original.max_frame_bytes, limits().max_frame_bytes);
    assert_eq!(original.response_timeout, initial_deadline);
    assert_eq!(original.outcome.unwrap().root, ready.root);
    assert!(!client.capacity().unwrap().control_over_capacity());
    let original = other_frame.wait().await.unwrap();
    assert_eq!(original.max_frame_bytes, limits().max_frame_bytes);
    assert_eq!(original.response_timeout, initial_deadline);
    assert_eq!(original.outcome.unwrap().root, ready.root);
    assert_eq!(client.capacity().unwrap().control_reserved_bytes, 0);
    assert_eq!(client.capacity().unwrap().control_requests, 0);
    assert_eq!(client.capacity().unwrap().max_retained_frame_bytes, 0);
    owner.request_termination();
    assert_eq!(owner.join().await.unwrap().kind, StopKind::Requested);
}

#[tokio::test]
async fn credited_frame_maximum_counts_duplicate_bounds_until_their_last_receipt() {
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
            .await
            .unwrap();
    let client = owner.client();
    let mut requirements = client.frame_requirements();
    let mut ids = Vec::new();
    for bound in [8192, 16384, 32768, 32768] {
        let ticket = client
            .clone()
            .try_submit_with_frame_limit(WorkerCommand::Inspect, bound)
            .unwrap();
        ids.push(ticket.id);
        drop(ticket);
    }
    // Completion alone cannot reclaim a credited old frame. The exact owner
    // collects each real reply, preserving duplicate maxima until the last one.
    for id in &ids {
        client.completions().ready(*id).await.unwrap();
    }
    assert_eq!(client.capacity().unwrap().max_retained_frame_bytes, 32768);
    let initial = *requirements.borrow_and_update();
    assert_eq!(initial.max_retained_frame_bytes, 32768);
    assert_eq!(initial.required_capacity(), limits().max_frame_bytes);
    for (index, bound, next_max, remaining) in [
        (2, 32768, 32768, 3),
        (3, 32768, 16384, 2),
        (1, 16384, 8192, 1),
        (0, 8192, 0, 0),
    ] {
        let receipt = client.completions().try_take(ids[index]).unwrap();
        assert!(receipt.transport_started);
        assert_eq!(receipt.max_frame_bytes, bound);
        assert_eq!(receipt.outcome.unwrap().root, ready.root);
        let observation = client.clone().capacity().unwrap();
        assert_eq!(observation.max_retained_frame_bytes, next_max);
        assert_eq!(observation.control_requests, remaining);
        assert_eq!(observation.configured_frame_bytes, limits().max_frame_bytes);
        assert_eq!(
            observation.required_frame_capacity(),
            limits().max_frame_bytes
        );
        if next_max == 32768 {
            assert!(!requirements.has_changed().unwrap());
        } else {
            tokio::time::timeout(Duration::from_secs(1), requirements.changed())
                .await
                .unwrap()
                .unwrap();
            let requirement = *requirements.borrow_and_update();
            assert_eq!(requirement.max_retained_frame_bytes, next_max);
            assert_eq!(requirement.configured_frame_bytes, limits().max_frame_bytes);
        }
    }
    assert_eq!(client.capacity().unwrap().control_reserved_bytes, 0);
    owner.request_termination();
    assert_eq!(owner.join().await.unwrap().kind, StopKind::Requested);
}

#[tokio::test]
async fn worker_ack_commits_rust_policy_before_child_progress_even_without_a_waiter() {
    use brassclaw_resources::LiveMontyTaskSettings;
    let selected = boot(SOURCE);
    let initial_settings = selected.task_settings;
    let initial_bounds = selected.bounds.values;
    let live = LiveMontyTaskSettings::new(initial_settings.into()).unwrap();
    let (mut owner, ready) = TransportOwner::start(worker(), selected, limits(), actor_limits())
        .await
        .unwrap();
    let client = owner.client();
    let admitted = exchange(
        &client,
        WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        },
    )
    .await;
    let task = admitted.admitted_task.unwrap();
    let compute = admitted.task_accounting[0].compute_time.unwrap();
    let next = TaskSettings {
        revision: 2,
        max_compute_time: Duration::from_secs(60),
        token_budgets_enabled: false,
    };
    let mut bounds = initial_bounds;
    bounds.max_value_bytes *= 2;
    let commits = Arc::new(AtomicUsize::new(0));
    let count = commits.clone();
    let rust = live.clone();
    let observed = client.live_vm_bounds();
    let publication = client
        .try_submit_runtime_publication(
            WorkerCommand::Recipe {
                command: RecipeCommand::UpdateRuntimeSettings {
                    expected_revision: 1,
                    settings: next,
                    values: bounds,
                    max_recipe_contexts: None,
                    adapter_reserve_bytes: None,
                    heap_update: None,
                    frame_update: None,
                },
            },
            Box::new(move |snapshot| {
                assert_eq!(snapshot.effective_task_settings, Some(next));
                assert_eq!(snapshot.vm_bounds, Some(bounds));
                assert_eq!(
                    *observed.borrow(),
                    initial_bounds,
                    "worker observation must not escape before Rust publication"
                );
                assert_eq!(rust.current().revision, 1);
                rust.publish(1, next.into()).unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .unwrap();
    let id = publication.id;
    drop(publication);
    // The queued child is real VM work, not a synthetic dispatch result. The
    // accepted publication remains owned despite the dropped HTTP-like waiter.
    let opened = client
        .try_submit(WorkerCommand::Recipe {
            command: RecipeCommand::Open { task, parent: None },
        })
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap();
    assert!(matches!(opened.recipe, Some(RecipeEvent::Opened { .. })));
    assert_eq!(opened.root, ready.root);
    assert_eq!(opened.effective_task_settings, Some(next));
    assert!(opened.task_accounting[0].compute_time.unwrap() >= compute);
    assert_eq!(TaskSettings::from(live.current()), next);
    assert_eq!(commits.load(Ordering::SeqCst), 1);
    assert_eq!(*client.live_vm_bounds().borrow(), bounds);
    let receipt = client.completions().wait(id).await.unwrap();
    assert!(receipt.transport_started);
    assert!(
        matches!(receipt.original_command().unwrap(), WorkerCommand::Recipe {
        command: RecipeCommand::UpdateRuntimeSettings { settings, .. }
    } if settings == next)
    );
    receipt.outcome.unwrap();
    owner.request_termination();
    let exit = owner.join().await.unwrap();
    assert_eq!(exit.kind, StopKind::Requested);
    assert!(exit.containment_error.is_none() && exit.reap_error.is_none());
}

#[tokio::test]
async fn rust_publication_failure_retains_actual_worker_ack_and_fences_queued_work() {
    for panic in [false, true] {
        let (mut owner, ready) =
            TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
                .await
                .unwrap();
        let client = owner.client();
        let next = TaskSettings {
            revision: 2,
            max_compute_time: Duration::from_secs(60),
            token_budgets_enabled: false,
        };
        let ticket = client
            .try_submit_runtime_publication(
                WorkerCommand::Recipe {
                    command: RecipeCommand::UpdateSettings {
                        expected_revision: 1,
                        settings: next,
                    },
                },
                Box::new(move |snapshot| {
                    assert_eq!(snapshot.effective_task_settings, Some(next));
                    if panic {
                        panic!("intentional synchronous publication failure");
                    }
                    Err(ActorFailure::AccountingUnavailable)
                }),
            )
            .unwrap();
        let queued = client.try_submit(WorkerCommand::Inspect).unwrap();
        let receipt = ticket.wait().await.unwrap();
        assert!(receipt.transport_started);
        assert!(
            matches!(receipt.original_command().unwrap(), WorkerCommand::Recipe {
            command: RecipeCommand::UpdateSettings { settings, .. }
        } if settings == next)
        );
        let error = receipt.outcome.unwrap_err();
        assert_eq!(error.kind, ProcessFailure::Protocol);
        let actual = error.snapshot.unwrap();
        assert_eq!(actual.root, ready.root);
        assert_eq!(actual.effective_task_settings, Some(next));
        assert!(matches!(actual.recipe, Some(RecipeEvent::SettingsUpdated)));
        let exit = owner.join().await.unwrap();
        assert_eq!(exit.kind, StopKind::TransportFailed);
        assert!(exit.exit_status.is_some());
        assert!(exit.containment_error.is_none() && exit.reap_error.is_none());
        let unstarted = queued.wait().await.unwrap();
        assert!(!unstarted.transport_started);
        assert!(matches!(
            unstarted.original_command().unwrap(),
            WorkerCommand::Inspect
        ));
        assert_eq!(
            unstarted.outcome.unwrap_err().kind,
            ProcessFailure::Terminal
        );
    }
}

#[tokio::test]
async fn worker_denial_never_commits_rust_or_accepts_a_non_settings_callback() {
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
            .await
            .unwrap();
    let client = owner.client();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let rejected = match client.try_submit_runtime_publication(
        WorkerCommand::Inspect,
        Box::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }),
    ) {
        Err(error) => error,
        Ok(_) => panic!("general commands cannot acquire a settings callback"),
    };
    assert_eq!(rejected.kind, ActorFailure::InvalidLimits);
    assert!(matches!(*rejected.command, WorkerCommand::Inspect));
    let admitted = exchange(
        &client,
        WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        },
    )
    .await;
    let opened = exchange(
        &client,
        WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task: admitted.admitted_task.unwrap(),
                parent: None,
            },
        },
    )
    .await;
    assert_eq!(ready.recipe_context_capacity.unwrap().active, 0);
    assert_eq!(opened.recipe_context_capacity.unwrap().active, 1);
    assert_eq!(
        opened.runtime_settings(),
        ready.runtime_settings(),
        "live child usage must not masquerade as a settings change"
    );
    let count = calls.clone();
    let receipt = client
        .try_submit_runtime_publication(
            WorkerCommand::Recipe {
                command: RecipeCommand::UpdateSettings {
                    expected_revision: 99,
                    settings: TaskSettings {
                        revision: 100,
                        max_compute_time: Duration::from_secs(60),
                        token_budgets_enabled: false,
                    },
                },
            },
            Box::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .unwrap()
        .wait()
        .await
        .unwrap();
    let error = receipt.outcome.unwrap_err();
    assert_eq!(
        error.kind,
        ProcessFailure::Vm(brassclaw_monty_host::VmFailure::SettingsRevisionConflict)
    );
    let denied = error.snapshot.unwrap();
    assert_eq!(denied.recipe_context_capacity.unwrap().active, 1);
    assert_eq!(denied.runtime_settings(), ready.runtime_settings());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let inspected = exchange(&client, WorkerCommand::Inspect).await;
    assert_eq!(
        inspected.effective_task_settings,
        ready.effective_task_settings
    );
    assert_eq!(inspected.root, ready.root);
    assert_eq!(inspected.lifecycle, Lifecycle::Ready);
    owner.request_termination();
    assert_eq!(owner.join().await.unwrap().kind, StopKind::Requested);
}

#[tokio::test]
async fn unsolicited_idle_worker_exit_notifies_and_reaps_without_another_command() {
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
            .await
            .unwrap();
    assert_eq!(ready.lifecycle, Lifecycle::Ready);
    let client = owner.client();
    assert_eq!(client.stop_kind().unwrap(), None);
    assert!(client.completions().outstanding().unwrap().is_empty());
    // Signal only the actual just-spawned owned native worker. There is no
    // substitute worker, transport response or pending RPC to discover death.
    let pid = owner.worker_process_id().unwrap();
    let status = tokio::process::Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .await
        .unwrap();
    assert!(status.success());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), client.stopped())
            .await
            .unwrap()
            .unwrap(),
        StopKind::TransportFailed
    );
    let exit = tokio::time::timeout(Duration::from_secs(2), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.kind, StopKind::TransportFailed);
    assert!(!exit.exit_status.unwrap().success());
    assert!(exit.containment_error.is_none());
    assert!(exit.reap_error.is_none());
    assert_eq!(
        exit.shutdown_failure.unwrap().kind,
        ProcessFailure::Transport
    );
    assert!(client.completions().outstanding().unwrap().is_empty());
    assert!(
        matches!(client.try_submit(WorkerCommand::Inspect),Err(error) if error.kind == ActorFailure::Closed)
    );
}

#[tokio::test]
async fn actual_root_failure_fences_instance_and_retains_undispatched_queue() {
    let source = "import asyncio\nasync def worker(worker_id):\n    task = await host.await_next_task(worker_id)\n    if task is None:\n        return\n    raise RuntimeError('root worker failure')\nawait asyncio.gather(*[worker(i) for i in range(worker_count)])";
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(source), limits(), actor_limits())
            .await
            .unwrap();
    let client = owner.client();
    let failed = client
        .try_submit(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .unwrap();
    let queued = client
        .try_submit(WorkerCommand::Boot { boot: boot(SOURCE) })
        .unwrap();
    let receipt = tokio::time::timeout(Duration::from_secs(15), failed.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(receipt.transport_started);
    assert!(
        matches!(receipt.original_command().unwrap(), WorkerCommand::Admit { task, .. }
        if task["conversation_id"] == json!("reborn-conv-opaque"))
    );
    let error = receipt.outcome.unwrap_err();
    assert_eq!(
        error.kind,
        ProcessFailure::Vm(brassclaw_monty_host::VmFailure::Python)
    );
    let snapshot = error.snapshot.unwrap();
    assert_eq!(snapshot.lifecycle, Lifecycle::Failed);
    let task = snapshot
        .admitted_task
        .expect("fatal admission retains its issued routing handle");
    assert_eq!(snapshot.task_accounting.len(), 1);
    assert_eq!(snapshot.task_accounting[0].task, task);
    assert!(snapshot.task_accounting[0].compute_time.unwrap() > Duration::ZERO);
    // The other worker's actual unresolved work wait stays available as
    // reconciliation evidence. Root failure is not its task completion.
    assert!(snapshot.outstanding.contains(&ready.work_waits[1].1));
    let exit = tokio::time::timeout(Duration::from_secs(15), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.kind, StopKind::InstanceFailed);
    assert!(exit.exit_status.is_some());
    assert!(exit.containment_error.is_none());
    assert!(exit.reap_error.is_none());
    let queued = queued.wait().await.unwrap();
    assert!(!queued.transport_started);
    assert!(matches!(
        queued.original_command().unwrap(),
        WorkerCommand::Boot { .. }
    ));
    assert_eq!(queued.outcome.unwrap_err().kind, ProcessFailure::Terminal);
    assert!(client.completions().outstanding().unwrap().is_empty());
}

#[tokio::test]
async fn dropped_turn_wait_retains_real_admission_and_does_not_kill_instance() {
    let (mut owner, ready) = TransportOwner::start(
        worker(),
        boot(SOURCE),
        limits(),
        ActorLimits {
            max_unclaimed: 1,
            ..actor_limits()
        },
    )
    .await
    .unwrap();
    let client = owner.client();
    let inbox = client.completions();
    let ticket = client
        .try_submit(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .unwrap();
    let id = ticket.id;
    // Single-thread runtime: the actor has not run yet. Poll the real waiter to
    // Pending, then cancel it. The accepted command must still be executed.
    {
        let mut waiter = Box::pin(ticket.wait());
        std::future::poll_fn(|cx| match waiter.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("actor cannot have run before yielding"),
        })
        .await;
    }
    drop(ticket);
    let full = match client.try_submit(WorkerCommand::Admit {
        key: ready.work_waits[1].1,
        task: task(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("abandoned receipt still occupies its admission credit"),
    };
    assert_eq!(full.kind, ActorFailure::Backpressure);
    assert!(matches!(*full.command, WorkerCommand::Admit { .. }));
    tokio::time::timeout(Duration::from_secs(15), inbox.ready(id))
        .await
        .unwrap()
        .unwrap();
    // An abandoned ordinary receipt cannot consume the reserved control lane.
    let stopping = exchange(&client, WorkerCommand::BeginShutdown).await;
    assert_eq!(stopping.lifecycle, Lifecycle::Stopping);
    let receipt = inbox.wait(id).await.unwrap();
    assert!(receipt.transport_started);
    assert!(
        matches!(receipt.original_command().unwrap(), WorkerCommand::Admit { task, .. } if task["conversation_id"] == json!("reborn-conv-opaque"))
    );
    let admitted = receipt.outcome.unwrap();
    assert_eq!(admitted.lifecycle, Lifecycle::Ready);
    assert!(admitted.admitted_task.is_some());
    let Some(ProcessBoundary::HostCall { key, name, .. }) = admitted.boundary else {
        panic!("actual intent port required")
    };
    assert_eq!(name, "resolve_intent");
    assert!(inbox.outstanding().unwrap().is_empty());
    let deferred = exchange(&client, WorkerCommand::Defer { key }).await;
    assert_eq!(deferred.lifecycle, Lifecycle::Stopping);
    assert_eq!(deferred.outstanding.len(), 2);
    owner.request_termination();
    let exit = tokio::time::timeout(Duration::from_secs(15), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.kind, StopKind::Requested);
    assert!(exit.exit_status.is_some());
    assert!(exit.reap_error.is_none());
    assert!(exit.containment_error.is_none());
    // Root intent/finish effects remain unresolved; local exit is not a durable
    // task completion and does not permit replay of the retained admission.
}

#[tokio::test]
async fn frame_credits_and_graceful_exit_reject_queued_work_with_full_evidence() {
    let (mut owner, ready) = TransportOwner::start(
        worker(),
        boot(SOURCE),
        limits(),
        ActorLimits {
            max_control_reserved_frame_bytes: limits().max_frame_bytes * 2,
            ..actor_limits()
        },
    )
    .await
    .unwrap();
    let client = owner.client();
    let ticket = client.try_submit(WorkerCommand::BeginShutdown).unwrap();
    let full = match client.try_submit(WorkerCommand::CloseWorker {
        key: ready.work_waits[0].1,
    }) {
        Err(error) => error,
        Ok(_) => panic!("response frame reservation must bound abandoned replies"),
    };
    assert_eq!(full.kind, ActorFailure::Backpressure);
    ticket.wait().await.unwrap().outcome.unwrap();
    exchange(
        &client,
        WorkerCommand::CloseWorker {
            key: ready.work_waits[0].1,
        },
    )
    .await;
    let last = client
        .try_submit(WorkerCommand::CloseWorker {
            key: ready.work_waits[1].1,
        })
        .unwrap();
    // This smaller budget admits one receipt at a time. Its credit is released
    // only after collection; the actual Stopped handshake closes admission.
    let stopped = last.wait().await.unwrap().outcome.unwrap();
    assert_eq!(stopped.lifecycle, Lifecycle::Stopped);
    let exit = tokio::time::timeout(Duration::from_secs(15), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.kind, StopKind::Graceful);
    assert!(exit.exit_status.unwrap().success());
    assert!(exit.shutdown_failure.is_none());
    let closed = match client.try_submit(WorkerCommand::Boot { boot: boot(SOURCE) }) {
        Err(error) => error,
        Ok(_) => panic!("stopped actor cannot accept replacement work"),
    };
    assert_eq!(closed.kind, ActorFailure::Closed);
    assert!(matches!(*closed.command, WorkerCommand::Boot { .. }));
}

#[tokio::test]
async fn real_native_worker_failure_retains_abandoned_result_and_undispatched_queue() {
    // Exercise fatal physical allocator containment, not recoverable logical
    // heap preflight. An instance ServiceOwner refuses this probe-only boot.
    let mut physical_probe = boot(SOURCE);
    physical_probe.heap_settings = None;
    let (mut owner, ready) =
        TransportOwner::start(worker(), physical_probe, limits(), actor_limits())
            .await
            .unwrap();
    let client = owner.client();
    let admitted = exchange(
        &client,
        WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        },
    )
    .await;
    let task = admitted.admitted_task.unwrap();
    let opened = exchange(
        &client,
        WorkerCommand::Recipe {
            command: RecipeCommand::Open { task, parent: None },
        },
    )
    .await;
    let Some(RecipeEvent::Opened { context, .. }) = opened.recipe else {
        panic!("actual child interpreter required")
    };
    let source = "result = 'x' * 67108864";
    let ticket = client
        .try_submit(WorkerCommand::Recipe {
            command: RecipeCommand::Start {
                context,
                selected: SelectedPython {
                    source: source.into(),
                    checksum: Sha256::digest(source.as_bytes()).into(),
                    aliases: Default::default(),
                },
                inputs: json!({}),
            },
        })
        .unwrap();
    let failed_id = ticket.id;
    drop(ticket);
    let queued = client
        .try_submit(WorkerCommand::Boot { boot: boot(SOURCE) })
        .unwrap();
    let receipt = tokio::time::timeout(
        Duration::from_secs(15),
        client.completions().wait(failed_id),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        receipt.outcome.as_ref().unwrap_err().kind,
        ProcessFailure::Transport
    );
    let exit = tokio::time::timeout(Duration::from_secs(15), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.kind, StopKind::TransportFailed);
    assert!(!exit.exit_status.unwrap().success());
    assert!(receipt.transport_started);
    assert!(
        matches!(receipt.original_command().unwrap(), WorkerCommand::Recipe { command: RecipeCommand::Start { selected, .. } } if selected.source == source)
    );
    assert_eq!(receipt.outcome.unwrap_err().kind, ProcessFailure::Transport);
    let not_started = queued.wait().await.unwrap();
    assert!(!not_started.transport_started);
    assert_eq!(
        not_started.outcome.as_ref().unwrap_err().kind,
        ProcessFailure::Terminal
    );
    assert!(matches!(
        not_started.original_command().unwrap(),
        WorkerCommand::Boot { .. }
    ));
    assert!(client.completions().outstanding().unwrap().is_empty());
}

#[tokio::test]
async fn real_child_python_failure_keeps_actor_and_root_available() {
    let (mut owner, ready) =
        TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
            .await
            .unwrap();
    let client = owner.client();
    let admitted = exchange(
        &client,
        WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        },
    )
    .await;
    let opened = exchange(
        &client,
        WorkerCommand::Recipe {
            command: RecipeCommand::Open {
                task: admitted.admitted_task.unwrap(),
                parent: None,
            },
        },
    )
    .await;
    let Some(RecipeEvent::Opened { context, .. }) = opened.recipe else {
        panic!("real context required")
    };
    let source = "result = 1 / 0";
    let ticket = client
        .try_submit(WorkerCommand::Recipe {
            command: RecipeCommand::Start {
                context,
                selected: SelectedPython {
                    source: source.into(),
                    checksum: Sha256::digest(source.as_bytes()).into(),
                    aliases: Default::default(),
                },
                inputs: json!({}),
            },
        })
        .unwrap();
    let failed = tokio::time::timeout(Duration::from_secs(15), ticket.wait())
        .await
        .unwrap()
        .unwrap();
    let error = failed.outcome.unwrap_err();
    assert_eq!(
        error.kind,
        ProcessFailure::Vm(brassclaw_monty_host::VmFailure::Python)
    );
    assert_eq!(error.snapshot.unwrap().lifecycle, Lifecycle::Ready);
    let stopping = exchange(&client, WorkerCommand::BeginShutdown).await;
    assert_eq!(stopping.lifecycle, Lifecycle::Stopping);
    owner.request_termination();
    let exit = tokio::time::timeout(Duration::from_secs(15), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.kind, StopKind::Requested);
    assert!(exit.exit_status.is_some());
}

#[tokio::test]
async fn live_actor_limits_keep_receipts_and_control_lane_across_reductions() {
    let initial = ActorLimits {
        max_unclaimed: 1,
        ..actor_limits()
    };
    let (mut owner, ready) = TransportOwner::start(worker(), boot(SOURCE), limits(), initial)
        .await
        .unwrap();
    let client = owner.client();
    let mut second_task = task();
    second_task["run_id"] = json!("run-b");
    second_task["turn_id"] = json!("turn-b");
    second_task["message_id"] = json!("message-b");
    let first = client
        .try_submit(WorkerCommand::Admit {
            key: ready.work_waits[0].1,
            task: task(),
        })
        .unwrap();
    assert!(matches!(client.try_submit(WorkerCommand::Admit {
        key: ready.work_waits[1].1, task: second_task.clone(),
    }), Err(error) if error.kind == ActorFailure::Backpressure));
    // A live increase must also remove the old physical channel ceiling.
    client
        .publish_limits(ActorLimits {
            max_unclaimed: 2048,
            ..initial
        })
        .unwrap();
    let second = client
        .try_submit(WorkerCommand::Admit {
            key: ready.work_waits[1].1,
            task: second_task,
        })
        .unwrap();
    client.publish_limits(initial).unwrap();
    let owned = client.capacity().unwrap();
    assert_eq!(owned.ordinary_requests, 2);
    assert!(owned.ordinary_over_capacity());
    // A separate control request remains admissible while ordinary debt exists.
    let inspected = client.try_submit(WorkerCommand::Inspect).unwrap();
    assert_eq!(client.capacity().unwrap().control_requests, 1);
    inspected.wait().await.unwrap().outcome.unwrap();
    assert_eq!(client.capacity().unwrap().control_requests, 0);
    first.wait().await.unwrap().outcome.unwrap();
    assert_eq!(client.capacity().unwrap().ordinary_requests, 1);
    assert!(!client.capacity().unwrap().ordinary_over_capacity());
    // Byte reductions retain the exact original wire and reservation.
    let reduced = ActorLimits {
        max_reserved_frame_bytes: limits().max_frame_bytes + 1,
        ..initial
    };
    client.publish_limits(reduced).unwrap();
    assert!(client.capacity().unwrap().ordinary_over_capacity());
    let id = second.id;
    drop(second);
    let receipt = client.completions().wait(id).await.unwrap();
    assert!(receipt.transport_started);
    assert!(matches!(
        receipt.original_command().unwrap(),
        WorkerCommand::Admit { .. }
    ));
    // The first admission leaves an unresolved root host boundary. The raw
    // actor does not orchestrate/defer it, so this queued second admission is
    // correctly rejected by the worker. Failed exchanges retain/refund the
    // same transport credits and exact command evidence as successful ones.
    assert!(matches!(receipt.outcome, Err(error) if error.kind ==
        ProcessFailure::Vm(brassclaw_monty_host::VmFailure::WrongBoundary)));
    assert_eq!(client.capacity().unwrap().ordinary_requests, 0);
    assert_eq!(client.capacity().unwrap().ordinary_reserved_bytes, 0);
    assert_eq!(
        client.publish_limits(ActorLimits {
            max_control_reserved_frame_bytes: limits().max_frame_bytes * 2 - 1,
            ..initial
        }),
        Err(ActorFailure::InvalidLimits)
    );
    assert_eq!(client.capacity().unwrap().limits, reduced);
    exchange(&client, WorkerCommand::BeginShutdown).await;
    owner.request_termination();
    let exit = owner.join().await.unwrap();
    assert_eq!(exit.kind, StopKind::Requested);
    assert_eq!(client.publish_limits(initial), Err(ActorFailure::Closed));
}

#[tokio::test]
async fn accepted_exchanges_retain_deadlines_across_live_hosting_changes() {
    use brassclaw_monty_host::transport_actor::HostingDeadlines;
    let (mut owner, _) = TransportOwner::start(worker(), boot(SOURCE), limits(), actor_limits())
        .await
        .unwrap();
    let client = owner.client();
    let initial = client.hosting_deadlines().unwrap();
    let old = client.try_submit(WorkerCommand::Inspect).unwrap();
    let grown = HostingDeadlines {
        startup_timeout: Duration::from_secs(2),
        response_timeout: Duration::from_secs(8),
    };
    client.publish_hosting_policy(None, Some(grown)).unwrap();
    assert_eq!(
        client.validate_execution_slice(Duration::from_secs(6)),
        Err(ActorFailure::Backpressure)
    );
    let after_growth = client.try_submit(WorkerCommand::Inspect).unwrap();
    let reduced = HostingDeadlines {
        response_timeout: Duration::from_secs(3),
        ..grown
    };
    client.publish_hosting_policy(None, Some(reduced)).unwrap();
    assert_eq!(client.hosting_deadlines().unwrap(), reduced);
    assert_eq!(
        client.validate_execution_slice(Duration::from_secs(3)),
        Err(ActorFailure::Backpressure)
    );
    // Completed receipt ownership is independent of a deadline update.
    let old = old.wait().await.unwrap();
    assert_eq!(old.response_timeout, initial.response_timeout);
    old.outcome.unwrap();
    let after_growth = after_growth.wait().await.unwrap();
    assert_eq!(after_growth.response_timeout, grown.response_timeout);
    after_growth.outcome.unwrap();
    let after_reduction = client
        .try_submit(WorkerCommand::Inspect)
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(after_reduction.response_timeout, reduced.response_timeout);
    after_reduction.outcome.unwrap();
    assert_eq!(
        client.publish_hosting_policy(
            None,
            Some(HostingDeadlines {
                startup_timeout: Duration::from_secs(4),
                ..reduced
            })
        ),
        Err(ActorFailure::InvalidLimits)
    );
    assert_eq!(client.hosting_deadlines().unwrap(), reduced);
    assert_eq!(
        client.validate_execution_slice(Duration::from_secs(2)),
        Ok(())
    );
    exchange(&client, WorkerCommand::BeginShutdown).await;
    owner.request_termination();
    assert_eq!(owner.join().await.unwrap().kind, StopKind::Requested);
}

#[tokio::test]
async fn startup_compile_deadline_is_contained_independently_of_ipc_response() {
    let source = format!("{}{}", "startup_value = 0\n".repeat(8000), SOURCE);
    let mut selected = boot(&source);
    selected.bounds.values.max_source_bytes = 192 * 1024;
    selected.bounds.values.max_compiled_source_bytes = 256 * 1024;
    selected.startup_timeout = Duration::from_millis(1);
    let result = TransportOwner::start(worker(), selected, limits(), actor_limits()).await;
    let error = match result {
        Err(brassclaw_monty_host::transport_actor::StartError::Worker(error)) => error,
        Err(error) => panic!("unexpected actor preflight error: {error}"),
        Ok(_) => panic!("late worker compilation cannot publish readiness"),
    };
    assert!(matches!(
        error.kind,
        ProcessFailure::Deadline
            | ProcessFailure::Vm(brassclaw_monty_host::VmFailure::StartupDeadline)
    ));
    assert!(error.exit_status.is_some());
    assert!(error.containment_error.is_none());
    assert!(error.reap_error.is_none());
    assert!(
        matches!(error.command.as_deref(), Some(WorkerCommand::Boot { boot })
        if boot.source == source && boot.startup_timeout == Duration::from_millis(1))
    );
}
