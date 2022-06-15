use std::sync::Arc;

enum ChannelType {
    Tcp,
    InProc,
    Udp,
}

pub struct ChannelHandle(Arc<ChannelType>);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_destroy(handle: *mut ChannelHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_channel_tcp_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Tcp))))
}

#[no_mangle]
pub extern "C" fn rsn_channel_inproc_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::InProc))))
}

#[no_mangle]
pub extern "C" fn rsn_channel_udp_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Udp))))
}
