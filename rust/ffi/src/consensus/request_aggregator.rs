use rsnano_node::consensus::{RequestAggregator, RequestAggregatorConfig};
use std::{ops::Deref, sync::Arc};

pub struct RequestAggregatorHandle(pub Arc<RequestAggregator>);

impl Deref for RequestAggregatorHandle {
    type Target = Arc<RequestAggregator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_request_aggregator_destroy(handle: *mut RequestAggregatorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_len(handle: &RequestAggregatorHandle) -> usize {
    handle.0.len()
}

#[repr(C)]
pub struct RequestAggregatorConfigDto {
    pub threads: usize,
    pub max_queue: usize,
    pub batch_size: usize,
}

impl From<&RequestAggregatorConfigDto> for RequestAggregatorConfig {
    fn from(value: &RequestAggregatorConfigDto) -> Self {
        Self {
            threads: value.threads,
            max_queue: value.max_queue,
            batch_size: value.batch_size,
        }
    }
}

impl From<&RequestAggregatorConfig> for RequestAggregatorConfigDto {
    fn from(value: &RequestAggregatorConfig) -> Self {
        Self {
            threads: value.threads,
            max_queue: value.max_queue,
            batch_size: value.batch_size,
        }
    }
}
