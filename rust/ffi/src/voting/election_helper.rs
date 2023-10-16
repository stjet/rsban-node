use std::{ops::Deref, sync::Arc};

use rsnano_core::Amount;
use rsnano_node::voting::ElectionHelper;

use crate::{representatives::OnlineRepsHandle, NetworkParamsDto};

pub struct ElectionHelperHandle(Arc<ElectionHelper>);

impl Deref for ElectionHelperHandle {
    type Target = Arc<ElectionHelper>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_election_helper_create(
    online_reps: &OnlineRepsHandle,
    network_params: &NetworkParamsDto,
) -> *mut ElectionHelperHandle {
    Box::into_raw(Box::new(ElectionHelperHandle(Arc::new(ElectionHelper {
        online_reps: Arc::clone(online_reps),
        network_params: network_params.try_into().unwrap(),
    }))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_helper_destroy(handle: *mut ElectionHelperHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_helper_base_latency_ms(handle: &ElectionHelperHandle) -> u64 {
    handle.base_latency().as_millis() as u64
}
