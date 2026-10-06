//! Install Monty's process-wide allocator only in this isolated test process.
//! Never link this allocator into the product as an alleged VM-local limit.

use std::{hint::black_box, process::ExitCode, time::Duration};

use monty::MontyRun;
use monty_types::{
    CompileOptions, PrintWriter, ResourceLimits, ResourceTracker, memory_limit_with_headroom,
};

#[global_allocator]
static ALLOCATOR: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("hard-limit") => {
            monty_alloc::set_hard_limit(Some(1024 * 1024)).expect("installed allocator");
            // No Monty preflight: prove that the allocator really contains a
            // between-checkpoint allocation by terminating only this process.
            black_box(vec![0_u8; 16 * 1024 * 1024]);
            eprintln!("allocator failed to enforce the hard limit");
            ExitCode::FAILURE
        }
        Some("soft-limit") => {
            const LIMIT: usize = 4 * 1024 * 1024;
            monty_alloc::set_hard_limit(memory_limit_with_headroom(Some(LIMIT), false))
                .expect("installed allocator");
            let run = MontyRun::new(
                "result = 'x' * 16777216\nresult".into(),
                "memory-probe.py",
                vec![],
                CompileOptions::default(),
            )
            .expect("valid probe source");
            let tracker = ResourceTracker::new(
                ResourceLimits::default()
                    .max_memory(LIMIT)
                    .max_feed_duration(Duration::from_secs(5)),
            );
            let mut stdout = String::new();
            match run.start(
                vec![],
                tracker,
                PrintWriter::CollectString(&mut stdout, Some(1024)),
            ) {
                Err(error) if error.to_string().to_lowercase().contains("memory") => {
                    println!("soft-memory-limit-enforced");
                    ExitCode::SUCCESS
                }
                _ => {
                    eprintln!("interpreter failed to enforce the soft memory limit");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("expected soft-limit or hard-limit");
            ExitCode::FAILURE
        }
    }
}
