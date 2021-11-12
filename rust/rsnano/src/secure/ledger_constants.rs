use crate::{
    config::{Networks, WorkThresholds},
    numbers::KeyPair,
};

pub struct LedgerConstants {
    pub work: WorkThresholds,
    pub zero_key: KeyPair,
}

impl LedgerConstants {
    pub fn new(work: WorkThresholds, _network: Networks) -> Self {
        Self {
            work,
            zero_key: KeyPair::zero(),
        }
    }
}
