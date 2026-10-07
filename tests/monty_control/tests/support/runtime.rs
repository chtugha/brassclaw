//! Real contained-worker boot fixture; no host-effect results are invented.
use brassclaw_monty_host::{
    VmBounds,
    global::GlobalBounds,
    heap::HeapSettings,
    process::{ProcessLimits, RootBoot, TaskSettings},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path, time::Duration};

pub(crate) fn boot(source: &str) -> RootBoot {
    RootBoot {
        heap_settings: Some(HeapSettings {
            revision: 1,
            max_vm_bytes: 16 * 1024 * 1024,
        }),
        source: source.to_owned(),
        checksum: Sha256::digest(source.as_bytes()).into(),
        aliases: [
            "await_next_task",
            "enter_task",
            "resolve_intent",
            "resolve_component_by_name",
            "compose_orchestrator",
            "run_program",
            "resolve_reply",
            "finish_task",
            "visible_capabilities",
            "build_prompt_bundle",
            "stream_model",
            "invoke_capability",
            "append_capability_result_ref",
            "post_reply",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        bounds: GlobalBounds {
            values: VmBounds {
                max_source_bytes: 32768,
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
        task_settings: TaskSettings {
            revision: 1,
            max_compute_time: Duration::from_secs(600),
            token_budgets_enabled: false,
        },
        max_recipe_contexts: 8,
    }
}
pub(crate) fn limits() -> ProcessLimits {
    ProcessLimits {
        hard_memory_bytes: 64 * 1024 * 1024,
        max_frame_bytes: 256 * 1024,
        response_timeout: Duration::from_secs(5),
    }
}
pub(crate) fn worker() -> &'static Path {
    static PACKAGED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    if let Some(path) = option_env!("CARGO_BIN_EXE_global_worker") {
        Path::new(path)
    } else {
        PACKAGED.get_or_init(|| {
            brassclaw_monty_host::process::installed_worker().expect("packaged actual Monty worker")
        })
    }
}
