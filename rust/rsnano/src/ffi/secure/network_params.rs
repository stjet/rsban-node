use std::convert::TryFrom;

use crate::{config::NetworkConstants, ffi::config::{NetworkConstantsDto, WorkThresholdsDto}, secure::{NetworkParams, PortmappingConstants}};

use super::{
    bootstrap_constants::BootstrapConstantsDto, ledger_constants::LedgerConstantsDto,
    node_constants::NodeConstantsDto, portmapping_constants::PortmappingConstantsDto,
    voting_constants::VotingConstantsDto,
};

#[repr(C)]
pub struct NetworkParamsDto {
    pub kdf_work: u32,
    pub work: WorkThresholdsDto,
    pub network: NetworkConstantsDto,
    pub ledger: LedgerConstantsDto,
    pub voting: VotingConstantsDto,
    pub node: NodeConstantsDto,
    pub portmapping: PortmappingConstantsDto,
    pub bootstrap: BootstrapConstantsDto,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_params_create(
    network_constants: &NetworkConstantsDto,
    dto: *mut NetworkParamsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    //let params = NetworkParams::new(&network_constants);
    //(*dto).kdf_work = params.kdf_work;
    //todo ...
    0
}
