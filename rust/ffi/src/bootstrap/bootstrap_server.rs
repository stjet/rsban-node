use rsnano_node::bootstrap::BootstrapServerConfig;

#[repr(C)]
pub struct BootstrapServerConfigDto {
    pub max_queue: usize,
    pub threads: usize,
    pub batch_size: usize,
}

impl From<&BootstrapServerConfig> for BootstrapServerConfigDto {
    fn from(value: &BootstrapServerConfig) -> Self {
        Self {
            max_queue: value.max_queue,
            threads: value.threads,
            batch_size: value.batch_size,
        }
    }
}

impl From<&BootstrapServerConfigDto> for BootstrapServerConfig {
    fn from(value: &BootstrapServerConfigDto) -> Self {
        Self {
            max_queue: value.max_queue,
            threads: value.threads,
            batch_size: value.batch_size,
        }
    }
}
