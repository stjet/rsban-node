mod block_processor;
pub(crate) use block_processor::*;
use rsnano_node::block_processing::BlockProcessorConfig;
mod backlog_population;
mod local_block_broadcaster;
mod unchecked_map;

pub use backlog_population::BacklogPopulationHandle;
pub use local_block_broadcaster::LocalBlockBroadcasterHandle;
pub use unchecked_map::UncheckedMapHandle;

#[repr(C)]
pub struct BlockProcessorConfigDto {
    pub max_peer_queue: usize,
    pub max_system_queue: usize,
    pub priority_live: usize,
    pub priority_bootstrap: usize,
    pub priority_local: usize,
}

impl From<&BlockProcessorConfigDto> for BlockProcessorConfig {
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

impl From<&BlockProcessorConfig> for BlockProcessorConfigDto {
    fn from(value: &BlockProcessorConfig) -> Self {
        Self {
            max_peer_queue: value.max_peer_queue,
            max_system_queue: value.max_system_queue,
            priority_live: value.priority_live,
            priority_bootstrap: value.priority_bootstrap,
            priority_local: value.priority_local,
        }
    }
}
