use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};
use std::{mem::size_of, slice};

pub struct Blake2bHandle(VarBlake2b);

#[no_mangle]
pub extern "C" fn rsn_blake2b_create(size: usize) -> *mut Blake2bHandle {
    let hasher = VarBlake2b::new(size).unwrap();
    Box::into_raw(Box::new(Blake2bHandle(hasher)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_blake2b_destroy(handle: *mut Blake2bHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_blake2b_update(
    handle: *mut Blake2bHandle,
    data: *const u8,
    size: usize,
) {
    let data = slice::from_raw_parts(data, size);
    (*handle).0.update(data);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_blake2b_final(
    handle: *mut Blake2bHandle,
    output: *mut u8,
    size: usize,
) {
    let output = slice::from_raw_parts_mut(output, size);
    let mut hasher = VarBlake2b::new(size_of::<u64>()).unwrap();
    std::mem::swap(&mut (*handle).0, &mut hasher);
    hasher.finalize_variable(|hash| output.copy_from_slice(hash));
}
