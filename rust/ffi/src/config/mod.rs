mod bootstrap_config;
mod daemon_config;
mod diagnostics_config;
mod lmdb_config;
mod network_constants;
mod node_config;
mod node_flags;
mod node_rpc_config;
mod opencl_config;
mod optimistic_scheduler_config;
mod rpc_config;
mod websocket_config;

pub use diagnostics_config::*;
pub use lmdb_config::LmdbConfigDto;
pub use network_constants::*;
pub use node_config::*;
pub use node_flags::NodeFlagsHandle;
pub use node_rpc_config::*;
pub use opencl_config::*;
pub use optimistic_scheduler_config::*;
pub use rpc_config::*;
use rsnano_node::config::BootstrapAscendingConfig;
pub use websocket_config::*;

#[repr(C)]
pub struct BlockProcessorConfigDto {
    pub max_peer_queue: usize,
    pub max_system_queue: usize,
    pub priority_live: usize,
    pub priority_bootstrap: usize,
    pub priority_local: usize,
}

impl From<&BlockProcessorConfigDto> for BootstrapAscendingConfig {
    fn from(value: &BlockProcessorConfigDto) -> Self {
        Self {
            max_peer_queue: value.max_peer_queue,
            max_system_queue: value.max_system_queue,
            priority_live: value.priority_live,
            priority_bootstrap: value.priority_bootstrap,
            priority_local: value.priority_local,
        }
    }
}

impl From<&BootstrapAscendingConfig> for BlockProcessorConfigDto {
    fn from(value: &BootstrapAscendingConfig) -> Self {
        Self {
            max_peer_queue: value.max_peer_queue,
            max_system_queue: value.max_system_queue,
            priority_live: value.priority_live,
            priority_bootstrap: value.priority_bootstrap,
            priority_local: value.priority_local,
        }
    }
}
