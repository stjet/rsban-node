use crate::{
    ledger::datastore::LedgerHandle, representatives::OnlineRepsHandle,
    utils::ContainerInfoComponentHandle, NetworkParamsDto, StatHandle,
};
use rsnano_core::Account;
use rsnano_node::consensus::RepTiers;
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
};

pub struct RepTiersHandle(pub Arc<RepTiers>);

impl Deref for RepTiersHandle {
    type Target = Arc<RepTiers>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_rep_tiers_create(
    ledger: &LedgerHandle,
    network_params: &NetworkParamsDto,
    online_reps: &OnlineRepsHandle,
    stats: &StatHandle,
) -> *mut RepTiersHandle {
    Box::into_raw(Box::new(RepTiersHandle(Arc::new(RepTiers::new(
        Arc::clone(ledger),
        network_params.try_into().unwrap(),
        Arc::clone(online_reps),
        Arc::clone(stats),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_tiers_destroy(handle: *mut RepTiersHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_rep_tiers_start(handle: &RepTiersHandle) {
    handle.start();
}

#[no_mangle]
pub extern "C" fn rsn_rep_tiers_stop(handle: &RepTiersHandle) {
    handle.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_tiers_tier(
    handle: &RepTiersHandle,
    representative: *const u8,
) -> u8 {
    handle.tier(&Account::from_ptr(representative)) as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_tiers_collect_container_info(
    handle: &RepTiersHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
