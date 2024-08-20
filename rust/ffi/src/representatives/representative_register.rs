use rsnano_core::Amount;
use rsnano_node::representatives::{OnlineReps, PeeredRep};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use super::representative::RepresentativeHandle;

pub struct RepresentativeRegisterHandle(pub Arc<Mutex<OnlineReps>>);

impl Deref for RepresentativeRegisterHandle {
    type Target = Arc<Mutex<OnlineReps>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_register_destroy(
    handle: *mut RepresentativeRegisterHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_register_total_weight(
    handle: &RepresentativeRegisterHandle,
    result: *mut u8,
) {
    let weight = handle.lock().unwrap().peered_weight();
    weight.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_register_representatives(
    handle: &RepresentativeRegisterHandle,
    max_results: usize,
    min_weight: *const u8,
) -> *mut RepresentativeListHandle {
    let min_weight = Amount::from_ptr(min_weight);

    let mut resp = handle.lock().unwrap().representatives_filter(min_weight);
    resp.truncate(max_results);

    Box::into_raw(Box::new(RepresentativeListHandle(resp)))
}

pub struct RepresentativeListHandle(Vec<PeeredRep>);

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_list_destroy(handle: *mut RepresentativeListHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_list_len(handle: &RepresentativeListHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_list_get(
    handle: &RepresentativeListHandle,
    index: usize,
) -> *mut RepresentativeHandle {
    let rep = handle.0.get(index).unwrap().clone();
    Box::into_raw(Box::new(RepresentativeHandle(rep)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_register_count(
    handle: &RepresentativeRegisterHandle,
) -> usize {
    handle.0.lock().unwrap().peered_reps_count()
}
