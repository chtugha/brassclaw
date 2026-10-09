#![doc = include_str!("../README.md")]
#![expect(
    unsafe_code,
    reason = "GlobalAlloc requires aligned private headers and matching System layouts"
)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    fmt,
    io::{self, Write},
    marker::PhantomData,
    process,
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[cfg(feature = "exit-code")]
use monty_types::OOM_EXIT_CODE;
use monty_types::{BASELINE_MEMORY, LIVE_MEMORY, VM_LIVE_MEMORY, VM_MEMORY_ACCOUNTING, VM_MEMORY_LIMIT};

/// The absolute ceiling for allocator-backed live bytes.
/// Counting starts with the process: a counter armed later would see `dealloc`s
/// it never charged and underflow.
static HARD_LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);

thread_local! {
    // Const initialization and a destructor-free Cell are essential: allocator
    // entry must neither allocate recursively nor acquire a mutex.
    static VM_SCOPE: Cell<bool> = const { Cell::new(false) };
}

/// Activate ownership accounting before creating any VM. This is irreversible
/// for the process: retained allocations must never lose their measurement.
pub fn enable_vm_accounting() -> Result<(), &'static str> {
    if LIVE_MEMORY.load(Ordering::Relaxed) == 0 {
        return Err("monty-alloc is not installed as the global allocator");
    }
    VM_MEMORY_ACCOUNTING.store(true, Ordering::Release);
    Ok(())
}

/// Synchronous allocation ownership, not a task or an interpreter lifetime.
/// Nested scopes restore their parent. The guard cannot move to another thread
/// or span a migrating async future; tags in each allocation outlive the guard.
#[must_use]
pub struct VmAllocationScope {
    previous: bool,
    _thread: PhantomData<Rc<()>>,
}
impl VmAllocationScope {
    pub fn enter() -> Result<Self, &'static str> {
        if !VM_MEMORY_ACCOUNTING.load(Ordering::Acquire) {
            return Err("VM allocation accounting is not enabled");
        }
        let previous = VM_SCOPE
            .try_with(|scope| scope.replace(true))
            .map_err(|_| "VM allocation context is unavailable")?;
        Ok(Self {
            previous,
            _thread: PhantomData,
        })
    }
}
impl Drop for VmAllocationScope {
    fn drop(&mut self) {
        // A destructing thread may no longer expose its TLS. Allocation headers
        // still carry ownership; deallocation never consults TLS.
        let _ = VM_SCOPE.try_with(|scope| scope.set(self.previous));
    }
}

/// Actual charged VM-domain bytes, including compilation, root/child heaps,
/// controlled host adaptation and retained execution bookkeeping. It excludes
/// unadopted incoming frames and outgoing JSON serialization. This is requested
/// allocator storage, not RSS or virtual memory.
#[must_use]
pub fn vm_live_bytes() -> usize {
    VM_LIVE_MEMORY.load(Ordering::Relaxed)
}

/// Publish a finite shared soft ceiling while interpreter execution is
/// quiescent. The serialized worker owner must exclude concurrent VM execution
/// and admission. This never resets consumption, ownership tags or hard limits.
/// Failed manual reductions preserve the last effective limit.
pub fn set_vm_limit(bytes: usize) -> Result<(), &'static str> {
    if !VM_MEMORY_ACCOUNTING.load(Ordering::Acquire) {
        return Err("VM allocation accounting is not enabled");
    }
    if bytes == 0 || bytes == usize::MAX {
        return Err("invalid VM memory limit");
    }
    if vm_live_bytes() > bytes {
        return Err("VM memory limit is below live allocations");
    }
    VM_MEMORY_LIMIT.store(bytes, Ordering::Relaxed);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerMemoryLimitError {
    InvalidSettings,
    AccountingUnavailable,
    Overflow,
    VmReduction,
    PhysicalReduction,
}
impl fmt::Display for WorkerMemoryLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSettings => "invalid worker memory limits",
            Self::AccountingUnavailable => "worker allocation accounting is unavailable",
            Self::Overflow => "allocator memory budget is out of range",
            Self::VmReduction => "VM memory limit is below live allocations",
            Self::PhysicalReduction => "worker memory limit is below live allocations",
        })
    }
}
impl std::error::Error for WorkerMemoryLimitError {}

/// Resize the logical VM ceiling and its finite allocator backstop together.
/// The serialized worker must be quiescent, with no concurrent allocation or
/// interpreter execution. All validation precedes publication; neither counter
/// nor ownership tag is reset. Transport and exception reserve is separate from
/// the VM budget and must cover the worker's bounded frame/adapter overhead.
pub fn set_worker_limits(vm_bytes: usize, reserve_bytes: usize) -> Result<(), WorkerMemoryLimitError> {
    let (baseline, hard) = checked_worker_limits(vm_bytes, reserve_bytes)?;
    // No fallible work or allocations between these stores. Only the quiescent
    // worker owner may call this function; another thread cannot use an interim
    // mixed ceiling. Baseline may decrease, never absorb live VM consumption.
    BASELINE_MEMORY.store(baseline, Ordering::Relaxed);
    HARD_LIMIT.store(hard, Ordering::Relaxed);
    VM_MEMORY_LIMIT.store(vm_bytes, Ordering::Relaxed);
    Ok(())
}

/// Check a proposed logical/physical layout against actual allocation ownership
/// without changing limits, counters or the baseline. This is an observation,
/// not a reservation: a caller needing durable feasibility must retain the
/// serialized quiescent worker boundary until publication or rejection.
pub fn validate_worker_limits(vm_bytes: usize, reserve_bytes: usize) -> Result<(), WorkerMemoryLimitError> {
    checked_worker_limits(vm_bytes, reserve_bytes).map(|_| ())
}

fn checked_worker_limits(vm_bytes: usize, reserve_bytes: usize) -> Result<(usize, usize), WorkerMemoryLimitError> {
    if !VM_MEMORY_ACCOUNTING.load(Ordering::Acquire) {
        return Err(WorkerMemoryLimitError::AccountingUnavailable);
    }
    let budget = vm_bytes
        .checked_add(reserve_bytes)
        .filter(|bytes| *bytes != usize::MAX && vm_bytes > 0 && reserve_bytes > 0)
        .ok_or(WorkerMemoryLimitError::InvalidSettings)?;
    let live = LIVE_MEMORY.load(Ordering::Relaxed);
    if live == 0 {
        return Err(WorkerMemoryLimitError::AccountingUnavailable);
    }
    let baseline = BASELINE_MEMORY.load(Ordering::Relaxed).min(live);
    let hard = baseline
        .checked_add(budget)
        .filter(|bytes| *bytes != usize::MAX)
        .ok_or(WorkerMemoryLimitError::Overflow)?;
    if vm_live_bytes() > vm_bytes {
        return Err(WorkerMemoryLimitError::VmReduction);
    }
    if live > hard {
        return Err(WorkerMemoryLimitError::PhysicalReduction);
    }
    Ok((baseline, hard))
}

#[derive(Clone, Copy)]
struct Header {
    vm_owned: bool,
}

fn storage_layout(payload: Layout) -> (Layout, usize) {
    Layout::new::<Header>()
        .extend(payload)
        .map(|(layout, offset)| (layout.pad_to_align(), offset))
        .unwrap_or_else(|_| out_of_memory(format_args!("monty worker: allocation layout overflow")))
}

fn allocation_is_vm_owned() -> bool {
    VM_MEMORY_ACCOUNTING.load(Ordering::Acquire) && VM_SCOPE.try_with(Cell::get).unwrap_or(false)
}

/// Applies a worker's hard memory budget relative to its baseline.
///
/// Callers choose any headroom above the interpreter's soft limit before
/// calling this function. Re-apply it after every request, since a session can
/// also arrive through a restored dump or end through a reset.
///
/// An unrepresentable finite ceiling is rejected; it must never become an
/// unlimited worker. Invalid settings leave the previously armed ceiling intact.
pub fn set_hard_limit(memory_budget: Option<usize>) -> Result<(), &'static str> {
    let live = LIVE_MEMORY.load(Ordering::Relaxed);
    if live == 0 {
        return Err("monty-alloc is not installed as the global allocator");
    }
    let candidate = BASELINE_MEMORY.load(Ordering::Relaxed).min(live);
    if memory_budget.is_some_and(|bytes| candidate.checked_add(bytes).is_none()) {
        return Err("allocator memory budget is out of range");
    }
    // `fetch_min` both reads and lowers the baseline: the first arming, on a
    // pristine worker, sets it, and a later leaner moment can only improve it.
    let baseline = BASELINE_MEMORY.fetch_min(live, Ordering::Relaxed).min(live);
    let hard_limit = match memory_budget {
        Some(bytes) => baseline
            .checked_add(bytes)
            .ok_or("allocator memory budget is out of range")?,
        None => usize::MAX,
    };
    HARD_LIMIT.store(hard_limit, Ordering::Relaxed);
    Ok(())
}

/// The system allocator, plus the live-byte count that enforces the memory
/// limit and a null check that ends the process deliberately.
pub struct LimitedAllocator;

// SAFETY: storage_layout reserves an aligned Header followed by a separately
// aligned payload. System receives only that combined layout and its base
// pointer. Callers receive only the payload, whose size/alignment are unchanged.
// The offset depends only on payload alignment, so realloc retains both the
// header and payload at their original offsets. Dealloc reconstructs exactly
// the original layout; headers are private and initialized before publication.
unsafe impl GlobalAlloc for LimitedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (storage, offset) = storage_layout(layout);
        let vm_owned = allocation_is_vm_owned();
        charge(storage.size(), vm_owned);
        // SAFETY: storage is a validated Layout containing header and payload.
        let ptr = unsafe { System.alloc(storage) };
        if ptr.is_null() {
            out_of_memory(format_args!(
                "monty worker: allocation of {} bytes failed",
                layout.size()
            ));
        }
        // SAFETY: storage reserves an aligned initialized header and a payload
        // at offset, with the caller's requested size and alignment.
        unsafe {
            ptr.cast::<Header>().write(Header { vm_owned });
            ptr.add(offset)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (storage, offset) = storage_layout(layout);
        // SAFETY: ptr is the live payload allocated with this layout. Its
        // initialized header lies exactly offset bytes before it.
        let base = unsafe { ptr.sub(offset) };
        // SAFETY: base is the aligned initialized private Header.
        let header = unsafe { base.cast::<Header>().read() };
        // SAFETY: base and storage reconstruct the exact System allocation.
        unsafe { System.dealloc(base, storage) };
        refund(storage.size(), header.vm_owned);
    }

    // Overridden rather than left to the default (which routes through `alloc`)
    // so `System` keeps using calloc's pre-zeroed pages.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let (storage, offset) = storage_layout(layout);
        let vm_owned = allocation_is_vm_owned();
        charge(storage.size(), vm_owned);
        // SAFETY: validated combined layout; the payload remains all zeroes.
        let ptr = unsafe { System.alloc_zeroed(storage) };
        if ptr.is_null() {
            out_of_memory(format_args!(
                "monty worker: allocation of {} bytes failed",
                layout.size()
            ));
        }
        // SAFETY: initializing the private header touches no payload bytes.
        unsafe {
            ptr.cast::<Header>().write(Header { vm_owned });
            ptr.add(offset)
        }
    }

    // Overridden for the same reason: the default reallocates and copies, while
    // `System` can often grow a block in place.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let (storage, offset) = storage_layout(layout);
        let new_payload = Layout::from_size_align(new_size, layout.align())
            .unwrap_or_else(|_| out_of_memory(format_args!("monty worker: allocation layout overflow")));
        let (new_storage, _) = storage_layout(new_payload);
        // SAFETY: reconstruct the live base/header from the caller's layout.
        let base = unsafe { ptr.sub(offset) };
        // SAFETY: our private header was initialized before returning ptr.
        let header = unsafe { base.cast::<Header>().read() };
        if new_storage.size() > storage.size() {
            charge(new_storage.size() - storage.size(), header.vm_owned);
        }
        // SAFETY: storage describes base's live System block. Both layouts have
        // identical alignment and header/payload offsets; nonzero new_size is
        // part of the GlobalAlloc caller contract.
        let new_ptr = unsafe { System.realloc(base, storage, new_storage.size()) };
        if new_ptr.is_null() {
            out_of_memory(format_args!("monty worker: allocation of {new_size} bytes failed"));
        }
        if new_storage.size() < storage.size() {
            refund(storage.size() - new_storage.size(), header.vm_owned);
        }
        // SAFETY: realloc preserves the initialized header and overlapping
        // payload. The original payload offset remains valid after resize.
        unsafe { new_ptr.add(offset) }
    }
}

/// Adds `size` to the live total, exiting past the hard limit.
#[inline]
fn charge(size: usize, vm_owned: bool) {
    let previous = LIVE_MEMORY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |live| live.checked_add(size))
        .unwrap_or_else(|_| out_of_memory(format_args!("monty worker: allocator accounting overflow")));
    let Some(live) = previous.checked_add(size) else {
        out_of_memory(format_args!("monty worker: allocator accounting overflow"));
    };
    if live > HARD_LIMIT.load(Ordering::Relaxed) {
        out_of_memory(format_args!(
            "monty worker: allocation of {size} bytes exceeds the memory limit"
        ));
    }
    if vm_owned {
        VM_LIVE_MEMORY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |live| live.checked_add(size))
            .unwrap_or_else(|_| out_of_memory(format_args!("monty worker: VM allocator accounting overflow")));
    }
}

/// Returns `size` to the live total. `Relaxed` throughout: the count only has to
/// be eventually right, and no other memory is published through it.
#[inline]
fn refund(size: usize, vm_owned: bool) {
    if LIVE_MEMORY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |live| live.checked_sub(size))
        .is_err()
    {
        out_of_memory(format_args!("monty worker: allocator accounting underflow"));
    }
    if vm_owned
        && VM_LIVE_MEMORY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |live| live.checked_sub(size))
            .is_err()
    {
        out_of_memory(format_args!("monty worker: VM allocator accounting underflow"));
    }
}

/// Reports why memory ran out and ends the process — never by panicking, whose
/// machinery allocates. How it ends is the `exit-code` feature's choice; see the
/// crate docs. Skipping destructors can leave a partial frame on the transport,
/// which the host already treats as a dead worker.
#[cold]
#[inline(never)]
fn out_of_memory(reason: fmt::Arguments<'_>) -> ! {
    // A genuinely exhausted host can fail the write and re-enter: let the first
    // caller write and send any re-entrant one straight to the end.
    static REPORTING: AtomicBool = AtomicBool::new(false);
    // Lift the limit first — writing to stderr allocates (the handle's lock),
    // which under an exceeded limit would re-enter and be silenced below,
    // losing the message. Safe because this path never returns.
    HARD_LIMIT.store(usize::MAX, Ordering::Relaxed);
    if !REPORTING.swap(true, Ordering::Relaxed) {
        let _ = writeln!(io::stderr(), "{reason}");
    }
    #[cfg(feature = "exit-code")]
    process::exit(OOM_EXIT_CODE);
    #[cfg(not(feature = "exit-code"))]
    process::abort();
}
