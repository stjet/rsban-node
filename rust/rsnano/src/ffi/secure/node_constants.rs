use std::convert::TryFrom;

use crate::{config::NetworkConstants, ffi::config::NetworkConstantsDto, secure::NodeConstants};

#[repr(C)]
pub struct NodeConstantsDto {
    pub backup_interval_m: i64,
    pub search_pending_interval_s: i64,
    pub unchecked_cleaning_interval_m: i64,
    pub process_confirmed_interval_ms: i64,
    pub max_weight_samples: u64,
    pub weight_period: u64,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_node_constants_create(
    network_constants: &NetworkConstantsDto,
    dto: *mut NodeConstantsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    let node = NodeConstants::new(&network_constants);
    (*dto).backup_interval_m = node.backup_interval_m;
    (*dto).search_pending_interval_s = node.search_pending_interval_s;
    (*dto).unchecked_cleaning_interval_m = node.unchecked_cleaning_interval_m;
    (*dto).process_confirmed_interval_ms = node.process_confirmed_interval_ms;
    (*dto).max_weight_samples = node.max_weight_samples;
    (*dto).weight_period = node.weight_period;
    0
}
