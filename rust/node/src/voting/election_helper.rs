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
    pub fn cooldown_time(&self, weight: Amount) -> Duration {
        let online_stake = { self.online_reps.lock().unwrap().trended() };
        if weight > online_stake / 20 {
            // Reps with more than 5% weight
            Duration::from_secs(1)
        } else if weight > online_stake / 100 {
            // Reps with more than 1% weight
            Duration::from_secs(5)
        } else {
            // The rest of smaller reps
            Duration::from_secs(15)
        }
    }

    pub fn base_latency(&self) -> Duration {
        if self.network_params.network.is_dev_network() {
            Duration::from_millis(25)
        } else {
            Duration::from_millis(1000)
        }
    }
}
