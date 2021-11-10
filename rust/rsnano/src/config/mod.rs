mod work_thresholds;

use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Mutex;
pub use work_thresholds::*;

//todo: make configurable in builld script again!
static ACTIVE_NETWORK: Lazy<Mutex<Networks>> = Lazy::new(|| Mutex::new(Networks::NanoDevNetwork));

pub struct NetworkConstants {}

impl NetworkConstants {
    pub fn active_network() -> Networks {
        *ACTIVE_NETWORK.lock().unwrap()
    }

    pub fn set_active_network(network: Networks) {
        *ACTIVE_NETWORK.lock().unwrap() = network;
    }

    pub fn set_active_network_from_str(network: impl AsRef<str>) -> Result<()> {
        let net = match network.as_ref() {
            "live" => Networks::NanoLiveNetwork,
            "beta" => Networks::NanoBetaNetwork,
            "dev" => Networks::NanoDevNetwork,
            "test" => Networks::NanoTestNetwork,
            _ => bail!("invalid network"),
        };
        Self::set_active_network(net);
        Ok(())
    }
}
