use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{Stat, StatConfig, StatDatapoint};

#[repr(C)]
pub struct StatConfigDto {
    pub sampling_enabled: bool,
    pub capacity: usize,
    pub interval: usize,
    pub log_interval_samples: usize,
    pub log_interval_counters: usize,
    pub log_rotation_count: usize,
    pub log_headers: bool,
    pub log_counters_filename: [u8; 128],
    pub log_counters_filename_len: usize,
    pub log_samples_filename: [u8; 128],
    pub log_samples_filename_len: usize,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_config_create(dto: *mut StatConfigDto) {
    let config = StatConfig::new();
    let dto = &mut (*dto);
    fill_stat_config_dto(dto, &config);
}

pub fn fill_stat_config_dto(dto: &mut StatConfigDto, config: &StatConfig) {
    dto.sampling_enabled = config.sampling_enabled;
    dto.capacity = config.capacity;
    dto.interval = config.interval;
    dto.log_interval_samples = config.log_interval_samples;
    dto.log_interval_counters = config.log_interval_counters;
    dto.log_rotation_count = config.log_rotation_count;
    dto.log_headers = config.log_headers;
    let bytes = config.log_counters_filename.as_bytes();
    dto.log_counters_filename[..bytes.len()].copy_from_slice(bytes);
    dto.log_counters_filename_len = bytes.len();
    let bytes = config.log_samples_filename.as_bytes();
    dto.log_samples_filename[..bytes.len()].copy_from_slice(bytes);
    dto.log_samples_filename_len = bytes.len();
}

impl From<&StatConfigDto> for StatConfig {
    fn from(dto: &StatConfigDto) -> Self {
        Self {
            sampling_enabled: dto.sampling_enabled,
            capacity: dto.capacity,
            interval: dto.interval,
            log_interval_samples: dto.log_interval_samples,
            log_interval_counters: dto.log_interval_counters,
            log_rotation_count: dto.log_rotation_count,
            log_headers: dto.log_headers,
            log_counters_filename: String::from_utf8_lossy(
                &dto.log_counters_filename[..dto.log_counters_filename_len],
            )
            .to_string(),
            log_samples_filename: String::from_utf8_lossy(
                &dto.log_samples_filename[..dto.log_samples_filename_len],
            )
            .to_string(),
        }
    }
}

pub struct StatHandle(Stat);

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_create(config: *const StatConfigDto) -> *mut StatHandle {
    Box::into_raw(Box::new(StatHandle(Stat::new(StatConfig::from(&*config)))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_destroy(handle: *mut StatHandle) {
    drop(Box::from_raw(handle))
}

pub struct StatDatapointHandle(StatDatapoint);

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_create() -> *mut StatDatapointHandle {
    Box::into_raw(Box::new(StatDatapointHandle(StatDatapoint::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_destroy(handle: *mut StatDatapointHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_clone(
    handle: *const StatDatapointHandle,
) -> *mut StatDatapointHandle {
    Box::into_raw(Box::new(StatDatapointHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_get_value(handle: *const StatDatapointHandle) -> u64 {
    (*handle).0.get_value()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_set_value(
    handle: *const StatDatapointHandle,
    value: u64,
) {
    (*handle).0.set_value(value);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_get_timestamp_ms(
    handle: *const StatDatapointHandle,
) -> u64 {
    (*handle)
        .0
        .get_timestamp()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_set_timestamp_ms(
    handle: *const StatDatapointHandle,
    timestamp_ms: u64,
) {
    (*handle).0.set_timestamp(
        SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_millis(timestamp_ms))
            .unwrap(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_datapoint_add(
    handle: *const StatDatapointHandle,
    addend: u64,
    update_timestamp: bool,
) {
    (*handle).0.add(addend, update_timestamp);
}
