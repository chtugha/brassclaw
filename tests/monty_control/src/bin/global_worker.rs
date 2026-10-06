//! Test entry point for the same real worker used by the hosting candidate.
#[global_allocator]
static ALLOCATOR: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    brassclaw_monty_host::process::worker_main()
}
