use core::ffi::{c_char, c_void};

pub const MPC_OK: i32 = 0;
pub const MPC_INVALID_ARGUMENT: i32 = -1;
pub const MPC_OPERATION_FAILED: i32 = -2;

pub type SessionStateCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, i32);
pub type SessionDataCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, usize);
pub type BrowserFoundCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char);
pub type BrowserLostCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);
pub type AdvertiserInvitationCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, usize) -> bool;

extern "C" {
    pub fn mpc_string_free(ptr: *mut c_char);
    pub fn mpc_object_release(ptr: *mut c_void);
    pub fn mpc_object_retain(ptr: *mut c_void) -> *mut c_void;
    pub fn mpc_ptr_array_free(ptr: *mut c_void);

    pub fn mpc_peer_id_create(
        display_name: *const c_char,
        error_out: *mut *mut c_char,
    ) -> *mut c_void;
    pub fn mpc_peer_id_display_name(peer: *mut c_void) -> *mut c_char;

    pub fn mpc_session_create_with_identity(
        peer: *mut c_void,
        identity_items: *const *mut c_void,
        identity_count: usize,
        encryption_preference: i32,
        error_out: *mut *mut c_char,
    ) -> *mut c_void;
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
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn mpc_session_send_resource(
        session: *mut c_void,
        file_path: *const c_char,
        resource_name: *const c_char,
        peer: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> *mut c_void;
    pub fn mpc_session_start_stream(
        session: *mut c_void,
        stream_name: *const c_char,
        peer: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> *mut c_void;
    pub fn mpc_session_disconnect(session: *mut c_void);
    pub fn mpc_progress_fraction_completed(progress: *mut c_void) -> f64;
    pub fn mpc_progress_is_finished(progress: *mut c_void) -> bool;
    pub fn mpc_output_stream_open(stream: *mut c_void);
    pub fn mpc_output_stream_close(stream: *mut c_void);
    pub fn mpc_output_stream_write(
        stream: *mut c_void,
        bytes: *const c_void,
        length: usize,
    ) -> isize;
    pub fn mpc_session_set_delegate(
        session: *mut c_void,
        context: *mut c_void,
        on_state: SessionStateCallback,
        on_data: SessionDataCallback,
    );
    pub fn mpc_session_clear_delegate(session: *mut c_void);

    pub fn mpc_browser_create(
        peer: *mut c_void,
        service_type: *const c_char,
        error_out: *mut *mut c_char,
    ) -> *mut c_void;
    pub fn mpc_browser_start(browser: *mut c_void);
    pub fn mpc_browser_stop(browser: *mut c_void);
    pub fn mpc_browser_invite_peer(
        browser: *mut c_void,
        peer: *mut c_void,
        session: *mut c_void,
        context_bytes: *const c_void,
        context_length: usize,
        timeout_seconds: f64,
    );
    pub fn mpc_browser_set_delegate(
        browser: *mut c_void,
        context: *mut c_void,
        on_found: BrowserFoundCallback,
        on_lost: BrowserLostCallback,
    );
    pub fn mpc_browser_clear_delegate(browser: *mut c_void);

    pub fn mpc_advertiser_create(
        peer: *mut c_void,
        discovery_info_json: *const c_char,
        service_type: *const c_char,
        error_out: *mut *mut c_char,
    ) -> *mut c_void;
    pub fn mpc_advertiser_start(advertiser: *mut c_void);
    pub fn mpc_advertiser_stop(advertiser: *mut c_void);
    pub fn mpc_advertiser_set_delegate(
        advertiser: *mut c_void,
        invitation_session: *mut c_void,
        context: *mut c_void,
        on_invitation: AdvertiserInvitationCallback,
    );
    pub fn mpc_advertiser_clear_delegate(advertiser: *mut c_void);
}
