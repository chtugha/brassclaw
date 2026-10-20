//! Embedded Oniguruma allocations use the final binary's Rust global allocator.
//! There is no budget or allocator installed here: isolated BrassClaw workers
//! already install LimitedAllocator; ordinary binaries retain their own allocator.
//! The private header restores the exact Rust Layout for C free/realloc calls.
use std::{
    alloc::{self, Layout},
    ffi::c_void,
    mem::{align_of, size_of},
    ptr,
};

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Header {
    size: usize,
}

fn layout(size: usize) -> Option<Layout> {
    Layout::from_size_align(size.checked_add(size_of::<Header>())?, align_of::<Header>()).ok()
}

unsafe fn allocate(size: usize, zeroed: bool) -> *mut c_void {
    let Some(layout) = layout(size) else {
        return ptr::null_mut();
    };
    // SAFETY: the checked nonzero layout reserves the header and C payload.
    let base = unsafe {
        if zeroed {
            alloc::alloc_zeroed(layout)
        } else {
            alloc::alloc(layout)
        }
    };
    if base.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: base has Header alignment and storage, followed by size bytes.
    unsafe {
        base.cast::<Header>().write(Header { size });
        base.add(size_of::<Header>()).cast()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn brassclaw_onig_malloc(size: usize) -> *mut c_void {
    // SAFETY: this function returns a fresh C allocation or null.
    unsafe { allocate(size, false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn brassclaw_onig_calloc(count: usize, size: usize) -> *mut c_void {
    let Some(bytes) = count.checked_mul(size) else {
        return ptr::null_mut();
    };
    // SAFETY: the full payload is zero-initialized; arithmetic overflow fails.
    unsafe { allocate(bytes, true) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn brassclaw_onig_free(payload: *mut c_void) {
    if payload.is_null() {
        return;
    }
    // SAFETY: embedded Oniguruma passes only a live pointer from these functions.
    let base = unsafe { payload.cast::<u8>().sub(size_of::<Header>()) };
    let header = unsafe { base.cast::<Header>().read() };
    // SAFETY: a live private header always reconstructs its original layout.
    unsafe {
        alloc::dealloc(
            base,
            layout(header.size).expect("live Oniguruma allocation layout"),
        )
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn brassclaw_onig_realloc(payload: *mut c_void, size: usize) -> *mut c_void {
    if payload.is_null() {
        return unsafe { allocate(size, false) };
    }
    if size == 0 {
        unsafe { brassclaw_onig_free(payload) };
        return ptr::null_mut();
    }
    let Some(new_layout) = layout(size) else {
        return ptr::null_mut();
    };
    // SAFETY: payload is a live private allocation. Rust realloc retains the
    // global allocator's original ownership tag, even outside a VM scope.
    let base = unsafe { payload.cast::<u8>().sub(size_of::<Header>()) };
    let header = unsafe { base.cast::<Header>().read() };
    let old_layout = layout(header.size).expect("live Oniguruma allocation layout");
    let resized = unsafe { alloc::realloc(base, old_layout, new_layout.size()) };
    if resized.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: successful realloc preserves the header/payload prefix and alignment.
    unsafe {
        resized.cast::<Header>().write(Header { size });
        resized.add(size_of::<Header>()).cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_layout_zeroing_reallocation_and_overflow_preserve_payload() {
        unsafe {
            assert!(brassclaw_onig_calloc(usize::MAX, 2).is_null());
            assert!(brassclaw_onig_malloc(usize::MAX).is_null());
            let initial = brassclaw_onig_calloc(8, 8).cast::<u8>();
            assert!(!initial.is_null());
            assert_eq!(initial as usize % 16, 0);
            assert!(std::slice::from_raw_parts(initial, 64)
                .iter()
                .all(|b| *b == 0));
            initial.write(71);
            initial.add(63).write(93);
            assert!(brassclaw_onig_realloc(initial.cast(), usize::MAX).is_null());
            assert_eq!(initial.read(), 71); // failed realloc leaves the old block live
            let larger = brassclaw_onig_realloc(initial.cast(), 1024).cast::<u8>();
            assert!(!larger.is_null());
            assert_eq!(larger.read(), 71);
            assert_eq!(larger.add(63).read(), 93);
            let smaller = brassclaw_onig_realloc(larger.cast(), 8).cast::<u8>();
            assert!(!smaller.is_null());
            assert_eq!(smaller.read(), 71);
            assert!(brassclaw_onig_realloc(smaller.cast(), 0).is_null());
            let empty = brassclaw_onig_malloc(0);
            assert!(!empty.is_null());
            brassclaw_onig_free(empty);
            brassclaw_onig_free(ptr::null_mut());
        }
    }
}
