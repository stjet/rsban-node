use crate::block_arrival::BlockArrival;

pub struct BlockArrivalHandle(BlockArrival);

#[no_mangle]
pub extern "C" fn rsn_block_arrival_create() -> *mut BlockArrivalHandle {
    Box::into_raw(Box::new(BlockArrivalHandle(BlockArrival::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_arrival_destroy(handle: *mut BlockArrivalHandle) {
    drop(Box::from_raw(handle))
}
