use core::ffi::{c_char, c_void};

pub type BrowserFoundCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char);
pub type BrowserLostCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);
pub type BrowserErrorCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

extern "C" {
    pub fn mpc_browser_create(peer: *mut c_void, service_type: *const c_char) -> *mut c_void;
    pub fn mpc_browser_copy_my_peer(browser: *mut c_void) -> *mut c_void;
    pub fn mpc_browser_service_type(browser: *mut c_void) -> *mut c_char;
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
        on_found: Option<BrowserFoundCallback>,
        on_lost: Option<BrowserLostCallback>,
        on_error: Option<BrowserErrorCallback>,
    );
    pub fn mpc_browser_clear_delegate(browser: *mut c_void);
}
