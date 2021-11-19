use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Mutex;

use super::{Networks, WorkThresholds};

//todo: make configurable in builld script again!
static ACTIVE_NETWORK: Lazy<Mutex<Networks>> = Lazy::new(|| Mutex::new(Networks::NanoDevNetwork));

pub struct NetworkConstants {
    pub work: WorkThresholds,
    // A representative is classified as principal based on its weight and this factor
    pub principal_weight_factor: u32,
    pub default_node_port: u16,
    pub default_rpc_port: u16,
    pub default_ipc_port: u16,
    pub default_websocket_port: u16,
    pub request_interval_ms: u32,
    pub cleanup_period_s: i64,
    /** Default maximum idle time for a socket before it's automatically closed */
    pub idle_timeout_s: i64,
    pub sync_cookie_cutoff_s: i64,
    pub bootstrap_interval_s: i64,
    /** Maximum number of peers per IP */
    pub max_peers_per_ip: usize,
    /** Maximum number of peers per subnetwork */
    pub max_peers_per_subnetwork: usize,
    pub peer_dump_interval_s: i64,

    pub current_network: Networks,
    /** Current protocol version */
    pub protocol_version: u8,
    /** Minimum accepted protocol version */
    pub protocol_version_min: u8,
}

impl NetworkConstants {
    pub fn new(work: WorkThresholds, network: Networks) -> Self {
        let cleanup_period_s = if network == Networks::NanoDevNetwork {
            1
        } else {
            60
        };
        let max_peers_per_ip = if network == Networks::NanoDevNetwork {
            10
        } else {
            5
        };
        Self {
            work,
            current_network: network,
            protocol_version: 0x12,
            protocol_version_min: 0x12,
            principal_weight_factor: 1000, // 0.1%
            default_node_port: Self::get_default_node_port(network),
            default_rpc_port: Self::get_default_rpc_port(network),
            default_ipc_port: Self::get_default_ipc_port(network),
            default_websocket_port: Self::get_default_websocket_port(network),
            request_interval_ms: if network == Networks::NanoDevNetwork {
                20
            } else {
                500
            },
            cleanup_period_s,
            idle_timeout_s: if network == Networks::NanoDevNetwork {
                cleanup_period_s * 15
            } else {
                cleanup_period_s * 2
            },
            sync_cookie_cutoff_s: 5,
            bootstrap_interval_s: 15 * 60,
            max_peers_per_ip,
            max_peers_per_subnetwork: max_peers_per_ip * 4,
            peer_dump_interval_s: if network == Networks::NanoDevNetwork {
                1
            } else {
                5 * 60
            },
        }
    }

    fn get_default_node_port(network: Networks) -> u16 {
        if network == Networks::NanoLiveNetwork {
            7075
        } else if network == Networks::NanoBetaNetwork {
            54000
        } else if network == Networks::NanoTestNetwork {
            test_node_port()
        } else {
            44000
        }
    }

    fn get_default_rpc_port(network: Networks) -> u16 {
        if network == Networks::NanoLiveNetwork {
            7076
        } else if network == Networks::NanoBetaNetwork {
            55000
        } else if network == Networks::NanoTestNetwork {
            test_rpc_port()
        } else {
            45000
        }
    }

    fn get_default_ipc_port(network: Networks) -> u16 {
        if network == Networks::NanoLiveNetwork {
            7077
        } else if network == Networks::NanoBetaNetwork {
            56000
        } else if network == Networks::NanoTestNetwork {
            test_ipc_port()
        } else {
            46000
        }
    }

    fn get_default_websocket_port(network: Networks) -> u16 {
        if network == Networks::NanoLiveNetwork {
            7078
        } else if network == Networks::NanoBetaNetwork {
            57000
        } else if network == Networks::NanoTestNetwork {
            test_websocket_port()
        } else {
            47000
        }
    }

    pub fn is_live_network(&self) -> bool {
        self.current_network == Networks::NanoLiveNetwork
    }

    pub fn is_beta_network(&self) -> bool {
        self.current_network == Networks::NanoBetaNetwork
    }

    pub fn is_dev_network(&self) -> bool {
        self.current_network == Networks::NanoDevNetwork
    }

    pub fn is_test_network(&self) -> bool {
        self.current_network == Networks::NanoTestNetwork
    }

    /** Initial value is ACTIVE_NETWORK compile flag, but can be overridden by a CLI flag */
    pub fn active_network() -> Networks {
        *ACTIVE_NETWORK.lock().unwrap()
    }

    /**
     * Optionally called on startup to override the global active network.
     * If not called, the compile-time option will be used.
     * @param network The new active network
     */
    pub fn set_active_network(network: Networks) {
        *ACTIVE_NETWORK.lock().unwrap() = network;
    }

    /**
     * Optionally called on startup to override the global active network.
     * If not called, the compile-time option will be used.
     * @param network The new active network. Valid values are "live", "beta" and "dev"
     */
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

    pub fn cleanup_period_half_ms(&self) -> i64 {
        (self.cleanup_period_s * 1000) / 2
    }

    pub fn cleanup_cutoff_s(&self) -> i64 {
        self.cleanup_period_s * 5
    }
}

fn get_env_or_default<T>(variable_name: &str, default: T) -> T
where
    T: core::str::FromStr + Copy,
{
    std::env::var(variable_name)
        .map(|v| v.parse::<T>().unwrap_or(default))
        .unwrap_or(default)
}

pub fn get_env_or_default_string(variable_name: &str, default: impl Into<String>) -> String {
    std::env::var(variable_name).unwrap_or_else(|_| default.into())
}

fn test_node_port() -> u16 {
    get_env_or_default("NANO_TEST_NODE_PORT", 17075)
}

fn test_rpc_port() -> u16 {
    get_env_or_default("NANO_TEST_RPC_PORT", 17076)
}

fn test_ipc_port() -> u16 {
    get_env_or_default("NANO_TEST_IPC_PORT", 17077)
}

fn test_websocket_port() -> u16 {
    get_env_or_default("NANO_TEST_WEBSOCKET_PORT", 17078)
}

