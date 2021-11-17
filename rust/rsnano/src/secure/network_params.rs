use crate::config::{NetworkConstants, Networks, WorkThresholds};

use super::{
    BootstrapConstants, LedgerConstants, NodeConstants, PortmappingConstants, VotingConstants,
};

pub struct NetworkParams {
    pub kdf_work: u32,
    pub work: WorkThresholds,
    pub network: NetworkConstants,
    pub ledger: LedgerConstants,
    pub voting: VotingConstants,
    pub node: NodeConstants,
    pub portmapping: PortmappingConstants,
    pub bootstrap: BootstrapConstants,
}

impl NetworkParams {
    pub fn new(network: Networks) -> Self {
        Self {
            kdf_work: todo!(),
            work: todo!(),
            network: todo!(),
            ledger: todo!(),
            voting: todo!(),
            node: todo!(),
            portmapping: todo!(),
            bootstrap: todo!(),
        }
    }
}
