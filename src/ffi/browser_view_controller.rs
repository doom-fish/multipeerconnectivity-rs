use core::ffi::{c_char, c_void};

pub type BrowserViewControllerCallback = unsafe extern "C" fn(*mut c_void);
pub type BrowserViewControllerShouldPresentCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> bool;

extern "C" {
    pub fn mpc_browser_view_controller_create_with_service_type(
        service_type: *const c_char,
        session: *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_browser_view_controller_create_with_browser(
        browser: *mut c_void,
        session: *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_browser_view_controller_copy_browser(controller: *mut c_void) -> *mut c_void;
    pub fn mpc_browser_view_controller_copy_session(controller: *mut c_void) -> *mut c_void;
    pub fn mpc_browser_view_controller_minimum_number_of_peers(controller: *mut c_void) -> usize;
    pub fn mpc_browser_view_controller_set_minimum_number_of_peers(
        controller: *mut c_void,
        value: usize,
    );
    pub fn mpc_browser_view_controller_maximum_number_of_peers(controller: *mut c_void) -> usize;
    pub fn mpc_browser_view_controller_set_maximum_number_of_peers(
        controller: *mut c_void,
        value: usize,
    );
    pub fn mpc_browser_view_controller_set_delegate(
        controller: *mut c_void,
        context: *mut c_void,
        on_finish: Option<BrowserViewControllerCallback>,
        on_cancel: Option<BrowserViewControllerCallback>,
        should_present: Option<BrowserViewControllerShouldPresentCallback>,
    );
    pub fn mpc_browser_view_controller_clear_delegate(controller: *mut c_void);
}
