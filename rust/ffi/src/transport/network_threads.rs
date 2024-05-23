use rsnano_node::transport::NetworkThreads;
use std::sync::{Arc, Mutex};

pub struct NetworkThreadsHandle(pub Arc<Mutex<NetworkThreads>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_destroy(handle: *mut NetworkThreadsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_start(handle: &NetworkThreadsHandle) {
    handle.0.lock().unwrap().start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_stop(handle: &NetworkThreadsHandle) {
    handle.0.lock().unwrap().stop();
}
