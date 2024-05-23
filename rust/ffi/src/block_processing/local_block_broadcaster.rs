use rsnano_node::block_processing::{LocalBlockBroadcaster, LocalBlockBroadcasterExt};
use std::sync::Arc;

pub struct LocalBlockBroadcasterHandle(pub Arc<LocalBlockBroadcaster>);

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_destroy(
    handle: *mut LocalBlockBroadcasterHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_start(handle: &LocalBlockBroadcasterHandle) {
    handle.0.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_stop(handle: &LocalBlockBroadcasterHandle) {
    handle.0.stop();
}
