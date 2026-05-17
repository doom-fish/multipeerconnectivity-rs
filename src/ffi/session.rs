use core::ffi::{c_char, c_void};

pub type SessionStateCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, i32);
pub type SessionDataCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, usize);
pub type SessionStreamCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char, *mut c_void);
pub type SessionResourceStartCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char, *mut c_void);
pub type SessionResourceFinishCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char, *const c_char, *mut c_void);
pub type SessionCertificateCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, usize) -> bool;
pub type ResourceSendCompletionCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

extern "C" {
    pub fn mpc_session_create_with_identity(
        peer: *mut c_void,
        identity_items: *const *mut c_void,
        identity_count: usize,
        encryption_preference: i32,
        error_out: *mut *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_session_create_with_identity_handles(
        peer: *mut c_void,
        identity_items: *const *mut c_void,
        identity_count: usize,
        encryption_preference: i32,
        error_out: *mut *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_session_copy_my_peer(session: *mut c_void) -> *mut c_void;
    pub fn mpc_session_copy_security_identity(
        session: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
    pub fn mpc_session_encryption_preference(session: *mut c_void) -> i32;
    pub fn mpc_session_copy_connected_peers(
        session: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
    pub fn mpc_session_send_data(
        session: *mut c_void,
        data: *const c_void,
        data_len: usize,
        peers: *const *mut c_void,
        peer_count: usize,
        mode: i32,
        error_out: *mut *mut c_void,
    ) -> i32;
    pub fn mpc_session_send_resource(
        session: *mut c_void,
        file_path: *const c_char,
        resource_name: *const c_char,
        peer: *mut c_void,
        context: *mut c_void,
        completion: Option<ResourceSendCompletionCallback>,
        error_out: *mut *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_session_start_stream(
        session: *mut c_void,
        stream_name: *const c_char,
        peer: *mut c_void,
        error_out: *mut *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_session_disconnect(session: *mut c_void);
    pub fn mpc_progress_fraction_completed(progress: *mut c_void) -> f64;
    pub fn mpc_progress_is_finished(progress: *mut c_void) -> bool;
    pub fn mpc_progress_completed_unit_count(progress: *mut c_void) -> i64;
    pub fn mpc_progress_total_unit_count(progress: *mut c_void) -> i64;
    pub fn mpc_output_stream_open(stream: *mut c_void);
    pub fn mpc_output_stream_close(stream: *mut c_void);
    pub fn mpc_output_stream_write(
        stream: *mut c_void,
        bytes: *const c_void,
        length: usize,
        error_out: *mut *mut c_void,
    ) -> isize;
    pub fn mpc_input_stream_open(stream: *mut c_void);
    pub fn mpc_input_stream_close(stream: *mut c_void);
    pub fn mpc_input_stream_has_bytes_available(stream: *mut c_void) -> bool;
    pub fn mpc_input_stream_read(
        stream: *mut c_void,
        bytes: *mut c_void,
        length: usize,
        error_out: *mut *mut c_void,
    ) -> isize;
    pub fn mpc_session_nearby_connection_data_for_peer(
        session: *mut c_void,
        peer: *mut c_void,
        out_bytes: *mut *mut c_void,
        out_len: *mut usize,
        error_out: *mut *mut c_void,
    ) -> i32;
    pub fn mpc_session_connect_peer(
        session: *mut c_void,
        peer: *mut c_void,
        nearby_connection_data: *const c_void,
        nearby_connection_data_len: usize,
    );
    pub fn mpc_session_cancel_connect_peer(session: *mut c_void, peer: *mut c_void);
    pub fn mpc_session_minimum_number_of_peers() -> usize;
    pub fn mpc_session_maximum_number_of_peers() -> usize;
    pub fn mpc_session_set_delegate(
        session: *mut c_void,
        context: *mut c_void,
        on_state: Option<SessionStateCallback>,
        on_data: Option<SessionDataCallback>,
        on_stream: Option<SessionStreamCallback>,
        on_resource_started: Option<SessionResourceStartCallback>,
        on_resource_finished: Option<SessionResourceFinishCallback>,
        on_certificate: Option<SessionCertificateCallback>,
    );
    pub fn mpc_session_clear_delegate(session: *mut c_void);
}
