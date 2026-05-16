use core::ffi::{c_char, c_void};

pub const MPC_ERROR_KIND_INVALID_ARGUMENT: i32 = 1;
pub const MPC_ERROR_KIND_OPERATION_FAILED: i32 = 2;
pub const MPC_ERROR_KIND_FRAMEWORK: i32 = 3;

extern "C" {
    pub fn mpc_error_kind(error: *mut c_void) -> i32;
    pub fn mpc_error_code(error: *mut c_void) -> i32;
    pub fn mpc_error_domain(error: *mut c_void) -> *mut c_char;
    pub fn mpc_error_description(error: *mut c_void) -> *mut c_char;
    pub fn mpc_mc_error_domain() -> *mut c_char;
}
