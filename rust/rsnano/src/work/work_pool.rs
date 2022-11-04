use std::time::Duration;

use crate::config::NetworkConstants;

pub struct WorkPool {
    network_constants: NetworkConstants,
    max_threads: u32,
    pow_rate_limiter: Duration,
}

impl WorkPool {
    pub fn new(
        network_constants: NetworkConstants,
        max_threads: u32,
        pow_rate_limiter: Duration,
    ) -> Self {
        Self {
            network_constants,
            max_threads,
            pow_rate_limiter,
        }
    }
}
