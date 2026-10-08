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
        "resize" | "resized-memory" => {
            use monty_alloc::WorkerMemoryLimitError;
            use std::sync::atomic::Ordering;
            monty_alloc::enable_vm_accounting()?;
            monty_alloc::set_worker_limits(1024 * 1024, 128 * 1024)?;
            let unrelated = vec![17_u8; 768 * 1024];
            assert_eq!(monty_alloc::vm_live_bytes(), 0);
            assert_eq!(
                monty_alloc::set_worker_limits(16 * 1024, 64 * 1024),
                Err(WorkerMemoryLimitError::PhysicalReduction)
            );
            assert_eq!(
                monty_types::VM_MEMORY_LIMIT.load(Ordering::Relaxed),
                1024 * 1024
            );
            drop(unrelated);
            monty_alloc::set_worker_limits(4 * 1024 * 1024, 128 * 1024)?;
            let owned = {
                let _scope = monty_alloc::VmAllocationScope::enter()?;
                vec![29_u8; 2 * 1024 * 1024]
            };
            let charged = monty_alloc::vm_live_bytes();
            assert!(charged >= 2 * 1024 * 1024);
            assert_eq!(
                monty_alloc::set_worker_limits(1024, 128 * 1024),
                Err(WorkerMemoryLimitError::VmReduction)
            );
            assert_eq!(
                monty_alloc::set_worker_limits(usize::MAX, 128 * 1024),
                Err(WorkerMemoryLimitError::InvalidSettings)
            );
            assert_eq!(monty_alloc::vm_live_bytes(), charged);
            assert_eq!(
                monty_types::VM_MEMORY_LIMIT.load(Ordering::Relaxed),
                4 * 1024 * 1024
            );
            assert_eq!(owned[owned.len() - 1], 29);
            drop(owned);
            assert_eq!(monty_alloc::vm_live_bytes(), 0);
            monty_alloc::set_worker_limits(512 * 1024, 128 * 1024)?;
            if mode == "resized-memory" {
                // Unowned native memory still reaches the resized physical cap.
                std::hint::black_box(vec![71_u8; 2 * 1024 * 1024]);
                return Err("native allocation escaped the resized hard limit".into());
            }
            println!("finite worker limits resized without resetting ownership");
        }
        "ownership" => {
            monty_alloc::enable_vm_accounting()?;
            assert_eq!(monty_alloc::vm_live_bytes(), 0);
            let mut transport = vec![13_u8; 4096];
            let (mut vm, aligned) = {
                let _outer = monty_alloc::VmAllocationScope::enter()?;
                let owned = vec![0_u8; 8192];
                let before = monty_alloc::vm_live_bytes();
                // A pre-existing unowned buffer keeps its ownership even when
                // resized by VM code. Accounting is attached to the allocation.
                transport.reserve_exact(8192);
                assert_eq!(monty_alloc::vm_live_bytes(), before);
                {
                    let _inner = monty_alloc::VmAllocationScope::enter()?;
                    let nested = vec![42_u16; 512];
                    assert!(monty_alloc::vm_live_bytes() >= before + 1024);
                    drop(nested);
                }
                assert_eq!(monty_alloc::vm_live_bytes(), before);
                #[repr(align(4096))]
                struct Aligned([u8; 4096]);
                let aligned = Box::new(Aligned([17; 4096]));
                assert_eq!((&*aligned as *const Aligned as usize) % 4096, 0);
                assert_eq!(aligned.0[4095], 17);
                (owned, aligned)
            };
            let owned_bytes = monty_alloc::vm_live_bytes();
            assert!(owned_bytes >= vm.capacity() + 4096);
            let unrelated = vec![0_u8; 16384];
            assert_eq!(monty_alloc::vm_live_bytes(), owned_bytes);
            drop(unrelated);
            assert_eq!(monty_alloc::vm_live_bytes(), owned_bytes);
            // Growth and shrink outside scope retain the original VM tag and
            // all initialized payload bytes across System reallocations.
            vm[0] = 91;
            let old_capacity = vm.capacity();
            vm.reserve_exact(8192);
            assert_eq!(vm[0], 91);
            assert_eq!(
                monty_alloc::vm_live_bytes(),
                owned_bytes + vm.capacity() - old_capacity
            );
            vm.truncate(32);
            vm.shrink_to_fit();
            assert_eq!(vm[0], 91);
            assert_eq!(
                monty_alloc::vm_live_bytes(),
                owned_bytes + vm.capacity() - old_capacity
            );
            drop(aligned);
            // Destruction on another thread must refund the original owner,
            // without TLS from the thread that made the allocation.
            std::thread::spawn(move || drop(vm)).join().unwrap();
            assert_eq!(monty_alloc::vm_live_bytes(), 0);
            assert!(transport.iter().all(|byte| *byte == 13));
            drop(transport);
            println!("VM allocation ownership preserved");
        }
        _ => return Err("unknown allocator probe mode".into()),
    }
    Ok(())
}
