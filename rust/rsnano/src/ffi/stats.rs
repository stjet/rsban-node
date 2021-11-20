use crate::stats::StatConfig;

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
