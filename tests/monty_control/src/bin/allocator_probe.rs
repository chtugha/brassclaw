//! Real isolated allocator worker; the application never installs this allocator
//! without the required process/ownership supervisor.
use monty::MontyRun;
use monty_types::{CompileOptions, PrintWriter, ResourceTracker};

#[global_allocator]
static ALLOCATOR: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args()
        .nth(1)
        .ok_or("missing allocator probe mode")?;
    // A live finite cap, rather than an unarmed/default allocator, is required.
    monty_alloc::set_hard_limit(Some(1024 * 1024))?;
    let overflow = monty_alloc::set_hard_limit(Some(usize::MAX));
    if overflow != Err("allocator memory budget is out of range") {
        return Err("unrepresentable memory budget was not rejected".into());
    }
    match mode.as_str() {
        "overflow" => println!("finite budget overflow rejected"),
        "memory" => {
            // The rejected settings must preserve the previous finite ceiling.
            // Default tracker deliberately has no soft max_memory: actual native
            // string allocation must be stopped by the installed hard allocator.
            let run = MontyRun::new(
                "'x' * 67108864".into(),
                "allocator.py",
                vec![],
                CompileOptions::default(),
            )?;
            let _result = run.start(vec![], ResourceTracker::default(), PrintWriter::Disabled)?;
            return Err("native allocation escaped the finite hard limit".into());
        }
        _ => return Err("unknown allocator probe mode".into()),
    }
    Ok(())
}
