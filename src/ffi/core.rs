use core::ffi::{c_char, c_void};

pub const MPC_OK: i32 = 0;
pub const MPC_INVALID_ARGUMENT: i32 = -1;
pub const MPC_OPERATION_FAILED: i32 = -2;

/// C trampoline handed to the Swift bridge for delegate context refcounting.
///
/// A delegate wrapper uses it to take (`retain`) or drop (`release`) a strong
/// reference on the Rust-owned delegate context. Retain is called from the
/// wrapper's `init`, release from its `deinit`, keeping the context alive for
/// the duration of any in-flight background-queue callback.
pub type ContextRetainCallback = unsafe extern "C" fn(*mut c_void);

extern "C" {
    pub fn mpc_string_free(ptr: *mut c_char);
    pub fn mpc_bytes_free(ptr: *mut c_void);
    pub fn mpc_object_release(ptr: *mut c_void);
    pub fn mpc_object_retain(ptr: *mut c_void) -> *mut c_void;
    pub fn mpc_ptr_array_free(ptr: *mut c_void);
}
