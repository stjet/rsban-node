use std::convert::TryFrom;

use crate::{
    config::NetworkConstants, ffi::config::NetworkConstantsDto, secure::BootstrapConstants,
};

#[repr(C)]
pub struct BootstrapConstantsDto {
    pub lazy_max_pull_blocks: u32,
    pub lazy_min_pull_blocks: u32,
    pub frontier_retry_limit: u32,
    pub lazy_retry_limit: u32,
    pub lazy_destinations_retry_limit: u32,
    pub gap_cache_bootstrap_start_interval_ms: i64,
    pub default_frontiers_age_seconds: u32,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_constants_create(
    network_constants: &NetworkConstantsDto,
    dto: *mut BootstrapConstantsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    let bootstrap = BootstrapConstants::new(&network_constants);
    let dto = &mut (*dto);
    fill_bootstrap_constants_dto(dto, &bootstrap);
    0
}

pub fn fill_bootstrap_constants_dto(
    dto: &mut BootstrapConstantsDto,
    bootstrap: &BootstrapConstants,
) {
    dto.lazy_max_pull_blocks = bootstrap.lazy_max_pull_blocks;
    dto.lazy_min_pull_blocks = bootstrap.lazy_min_pull_blocks;
    dto.frontier_retry_limit = bootstrap.frontier_retry_limit;
    dto.lazy_retry_limit = bootstrap.lazy_retry_limit;
    dto.lazy_destinations_retry_limit = bootstrap.lazy_destinations_retry_limit;
    dto.gap_cache_bootstrap_start_interval_ms = bootstrap.gap_cache_bootstrap_start_interval_ms;
    dto.default_frontiers_age_seconds = bootstrap.default_frontiers_age_seconds;
}

impl From<&BootstrapConstantsDto> for BootstrapConstants {
    fn from(dto: &BootstrapConstantsDto) -> Self {
        Self {
            lazy_max_pull_blocks: dto.lazy_max_pull_blocks,
            lazy_min_pull_blocks: dto.lazy_min_pull_blocks,
            frontier_retry_limit: dto.frontier_retry_limit,
            lazy_retry_limit: dto.lazy_retry_limit,
            lazy_destinations_retry_limit: dto.lazy_destinations_retry_limit,
            gap_cache_bootstrap_start_interval_ms: dto.gap_cache_bootstrap_start_interval_ms,
            default_frontiers_age_seconds: dto.default_frontiers_age_seconds,
        }
    }
}
