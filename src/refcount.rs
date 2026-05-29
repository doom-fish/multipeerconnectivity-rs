//! Shared reference counting for delegate context boxes passed across FFI.
//!
//! Each delegate state struct (and the async subscription sender wrapper)
//! embeds a [`RefCount`] so that the Rust owner *and* the Swift delegate object
//! can each hold a strong reference. The heap-allocated context is only freed
//! once both sides have released.
//!
//! This prevents a use-after-free: `MultipeerConnectivity` delegate callbacks
//! fire on a background dispatch queue, so a callback can still be in flight at
//! the exact moment Rust tears the delegate down. Swift takes a `+1` reference
//! in the delegate wrapper's `init` (via the [`retain`] trampoline) and drops
//! it in `deinit` (via [`release`]). ARC keeps the wrapper — and therefore its
//! context reference — alive for the duration of any in-flight callback, so the
//! `Box` is only freed when every holder has released.

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{fence, AtomicUsize, Ordering};

/// Reference counter embedded in every FFI delegate context.
pub struct RefCount(AtomicUsize);

impl RefCount {
    /// Creates a counter holding the single initial (Rust-owned) reference.
    pub const fn new() -> Self {
        Self(AtomicUsize::new(1))
    }
}

/// Implemented by context structs allocated via `Box::into_raw` and shared with
/// the Swift bridge through an opaque `*mut c_void` pointer.
pub trait RefCounted {
    fn ref_count(&self) -> &RefCount;
}

/// Increments the reference count of the context behind `ptr`.
///
/// # Safety
///
/// `ptr` must be null or point to a live `T` previously allocated via
/// `Box::into_raw`.
pub unsafe fn retain<T: RefCounted>(ptr: *mut c_void) {
    if let Some(ctx) = NonNull::new(ptr.cast::<T>()) {
        unsafe { ctx.as_ref() }
            .ref_count()
            .0
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Decrements the reference count of the context behind `ptr`, freeing the
/// backing `Box<T>` once it reaches zero.
///
/// # Safety
///
/// `ptr` must be null or point to a live `T` previously allocated via
/// `Box::into_raw`. Each call must balance a reference taken earlier (the
/// initial reference from [`RefCount::new`] or one added by [`retain`]).
pub unsafe fn release<T: RefCounted>(ptr: *mut c_void) {
    let Some(ctx) = NonNull::new(ptr.cast::<T>()) else {
        return;
    };
    let prev = unsafe { ctx.as_ref() }
        .ref_count()
        .0
        .fetch_sub(1, Ordering::Release);
    if prev == 1 {
        // Acquire fence pairs with the Release stores above from every other
        // thread that held a reference, so the freeing thread observes all
        // their writes before the `Box` is dropped. This is the canonical
        // `Arc`-style drop pattern; the fence is required for soundness on
        // weakly-ordered architectures (e.g. AArch64).
        fence(Ordering::Acquire);
        drop(unsafe { Box::from_raw(ctx.as_ptr()) });
    }
}
