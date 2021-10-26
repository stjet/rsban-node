use crate::blocks::ReceiveBlock;

pub struct ReceiveBlockHandle {
    block: ReceiveBlock,
}

#[repr(C)]
pub struct ReceiveBlockDto {
    pub work: u64,
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_create(dto: &ReceiveBlockDto) -> *mut ReceiveBlockHandle {
    Box::into_raw(Box::new(ReceiveBlockHandle {
        block: ReceiveBlock { work: dto.work },
    }))
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_clone(handle: &ReceiveBlockHandle) -> *mut ReceiveBlockHandle {
    Box::into_raw(Box::new(ReceiveBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_destroy(handle: *mut ReceiveBlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_work_set(handle: *mut ReceiveBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_work(handle: &ReceiveBlockHandle) -> u64 {
    handle.block.work
}
