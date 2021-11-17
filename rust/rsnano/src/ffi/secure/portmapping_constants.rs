use std::convert::TryFrom;

use crate::{
    config::NetworkConstants, ffi::config::NetworkConstantsDto, secure::PortmappingConstants,
};

#[repr(C)]
pub struct PortmappingConstantsDto {
    pub lease_duration_s: i64,
    pub health_check_period_s: i64,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_portmapping_constants_create(
    network_constants: &NetworkConstantsDto,
    dto: *mut PortmappingConstantsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    let mapping = PortmappingConstants::new(&network_constants);
    (*dto).lease_duration_s = mapping.lease_duration_s;
    (*dto).health_check_period_s = mapping.health_check_period_s;
    0
}
