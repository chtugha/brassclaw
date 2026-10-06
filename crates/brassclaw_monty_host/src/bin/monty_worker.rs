//! Isolated allocator/VM worker. Never install this allocator in the product.
#[global_allocator]
static ALLOCATOR: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    brassclaw_monty_host::process::worker_main()
}
