use core::ffi::{c_char, c_void};

pub type AdvertiserAssistantCallback = unsafe extern "C" fn(*mut c_void);

extern "C" {
    pub fn mpc_advertiser_assistant_create(
        service_type: *const c_char,
        discovery_info_json: *const c_char,
        session: *mut c_void,
    ) -> *mut c_void;
    pub fn mpc_advertiser_assistant_copy_session(assistant: *mut c_void) -> *mut c_void;
    pub fn mpc_advertiser_assistant_discovery_info_json(assistant: *mut c_void) -> *mut c_char;
    pub fn mpc_advertiser_assistant_service_type(assistant: *mut c_void) -> *mut c_char;
    pub fn mpc_advertiser_assistant_start(assistant: *mut c_void);
    pub fn mpc_advertiser_assistant_stop(assistant: *mut c_void);
    pub fn mpc_advertiser_assistant_set_delegate(
        assistant: *mut c_void,
        context: *mut c_void,
        on_will_present: Option<AdvertiserAssistantCallback>,
        on_did_dismiss: Option<AdvertiserAssistantCallback>,
    );
    pub fn mpc_advertiser_assistant_clear_delegate(assistant: *mut c_void);
}
