use core::ffi::{c_char, c_void};

pub const MPC_OK: i32 = 0;
pub const MPC_INVALID_ARGUMENT: i32 = -1;
pub const MPC_OPERATION_FAILED: i32 = -2;

extern "C" {
    pub fn mpc_string_free(ptr: *mut c_char);
    pub fn mpc_bytes_free(ptr: *mut c_void);
    pub fn mpc_object_release(ptr: *mut c_void);
    pub fn mpc_object_retain(ptr: *mut c_void) -> *mut c_void;
    pub fn mpc_ptr_array_free(ptr: *mut c_void);
}
