use super::VoteHandle;
use rsnano_node::consensus::VoteProcessorConfig;
use std::ffi::c_void;

pub type VoteProcessorVoteProcessedCallback =
    unsafe extern "C" fn(*mut c_void, *mut VoteHandle, u8, u8);

#[repr(C)]
pub struct VoteProcessorConfigDto {
    pub max_pr_queue: usize,
    pub max_non_pr_queue: usize,
    pub pr_priority: usize,
    pub threads: usize,
    pub batch_size: usize,
    pub max_triggered: usize,
}

impl From<&VoteProcessorConfigDto> for VoteProcessorConfig {
    fn from(value: &VoteProcessorConfigDto) -> Self {
        Self {
            max_pr_queue: value.max_pr_queue,
            max_non_pr_queue: value.max_non_pr_queue,
            pr_priority: value.pr_priority,
            threads: value.threads,
            batch_size: value.batch_size,
            max_triggered: value.max_triggered,
        }
    }
}

impl From<&VoteProcessorConfig> for VoteProcessorConfigDto {
    fn from(value: &VoteProcessorConfig) -> Self {
        Self {
            max_pr_queue: value.max_pr_queue,
            max_non_pr_queue: value.max_non_pr_queue,
            pr_priority: value.pr_priority,
            threads: value.threads,
            batch_size: value.batch_size,
            max_triggered: value.max_triggered,
        }
    }
}
