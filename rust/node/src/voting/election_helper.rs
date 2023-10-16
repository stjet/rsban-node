use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use rsnano_core::Amount;

use crate::{NetworkParams, OnlineReps};

pub struct ElectionHelper {
    pub online_reps: Arc<Mutex<OnlineReps>>,
    pub network_params: NetworkParams,
}

impl ElectionHelper {
    pub fn base_latency(&self) -> Duration {
        if self.network_params.network.is_dev_network() {
            Duration::from_millis(25)
        } else {
            Duration::from_millis(1000)
        }
    }
}
