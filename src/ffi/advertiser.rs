use core::ffi::{c_char, c_void};

pub type AdvertiserInvitationCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, usize) -> *mut c_void;
pub type AdvertiserErrorCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

extern "C" {
    pub fn mpc_advertiser_create(
        peer: *mut c_void,
        discovery_info_json: *const c_char,
        service_type: *const c_char,
    ) -> *mut c_void;
    pub fn mpc_advertiser_copy_my_peer(advertiser: *mut c_void) -> *mut c_void;
    pub fn mpc_advertiser_discovery_info_json(advertiser: *mut c_void) -> *mut c_char;
    pub fn mpc_advertiser_service_type(advertiser: *mut c_void) -> *mut c_char;
    pub fn mpc_advertiser_start(advertiser: *mut c_void);
    pub fn mpc_advertiser_stop(advertiser: *mut c_void);
    pub fn mpc_advertiser_set_delegate(
        advertiser: *mut c_void,
        context: *mut c_void,
        on_invitation: Option<AdvertiserInvitationCallback>,
        on_error: Option<AdvertiserErrorCallback>,
    );
    pub fn mpc_advertiser_clear_delegate(advertiser: *mut c_void);
}
