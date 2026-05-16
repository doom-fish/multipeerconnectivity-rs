use core::ffi::{c_char, c_void};

extern "C" {
    pub fn mpc_peer_id_create(
        display_name: *const c_char,
        error_out: *mut *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_peer_id_display_name(peer: *mut c_void) -> *mut c_char;
    pub fn mpc_peer_id_archive(
        peer: *mut c_void,
        out_bytes: *mut *mut c_void,
        out_len: *mut usize,
        error_out: *mut *mut c_void,
    ) -> i32;
    pub fn mpc_peer_id_from_archived_data(
        bytes: *const c_void,
        len: usize,
        error_out: *mut *mut c_void,
    ) -> *mut c_void;
}
