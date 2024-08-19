use rsnano_node::consensus::RequestAggregatorConfig;

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
