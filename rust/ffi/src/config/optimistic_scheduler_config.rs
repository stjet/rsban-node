use rsnano_node::config::OptimisticSchedulerConfig;

#[repr(C)]
pub struct OptimisticSchedulerConfigDto {
    pub enabled: bool,
    pub gap_threshold: usize,
    pub max_size: usize,
}

impl From<&OptimisticSchedulerConfigDto> for OptimisticSchedulerConfig {
    fn from(value: &OptimisticSchedulerConfigDto) -> Self {
        Self {
            enabled: value.enabled,
            gap_threshold: value.gap_threshold,
            max_size: value.max_size,
        }
    }
}

impl From<&OptimisticSchedulerConfig> for OptimisticSchedulerConfigDto {
    fn from(value: &OptimisticSchedulerConfig) -> Self {
        Self {
            enabled: value.enabled,
            gap_threshold: value.gap_threshold,
            max_size: value.max_size,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_optimistic_scheduler_config_create(
    dto: *mut OptimisticSchedulerConfigDto,
) {
    *dto = (&OptimisticSchedulerConfig::new()).into()
}
