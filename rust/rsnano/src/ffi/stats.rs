use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use num::FromPrimitive;

use crate::{
    stat_detail_as_str, stat_dir_as_str, stat_type_as_str, DetailType, FileWriter, JsonWriter,
    Stat, StatConfig, StatDatapoint, StatLogSink,
};

use super::{FfiPropertyTreeWriter, StringDto};

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

#[repr(C)]
pub struct HistogramBinDto {
    pub start_inclusive: u64,
    pub end_exclusive: u64,
    pub value: u64,
    pub timestamp_ms: u64,
}

pub struct HistogramsBinHandle(Vec<HistogramBinDto>);

#[repr(C)]
pub struct HistogramBinsDto {
    bins: *const HistogramBinDto,
    len: usize,
    handle: *mut HistogramsBinHandle,
}

pub struct StatLogSinkHandle(Box<dyn StatLogSink>);

#[no_mangle]
pub unsafe extern "C" fn rsn_file_writer_create(filename: *const i8) -> *mut StatLogSinkHandle {
    let filename = CStr::from_ptr(filename).to_str().unwrap();
    Box::into_raw(Box::new(StatLogSinkHandle(Box::new(
        FileWriter::new(filename).unwrap(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_json_writer_create() -> *mut StatLogSinkHandle {
    Box::into_raw(Box::new(StatLogSinkHandle(Box::new(JsonWriter::new()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_destroy(handle: *mut StatLogSinkHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_begin(handle: *mut StatLogSinkHandle) {
    (*handle).0.begin().unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_finalize(handle: *mut StatLogSinkHandle) {
    (*handle).0.finalize();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_write_header(
    handle: *mut StatLogSinkHandle,
    header: *const c_char,
    time_ms: u64,
) {
    let header = CStr::from_ptr(header).to_string_lossy();
    let wall_time = UNIX_EPOCH + Duration::from_millis(time_ms);
    (*handle).0.write_header(&header, wall_time).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_rotate(handle: *mut StatLogSinkHandle) {
    (*handle).0.rotate().unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_entries(handle: *mut StatLogSinkHandle) -> usize {
    (*handle).0.entries()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_inc_entries(handle: *mut StatLogSinkHandle) {
    (*handle).0.inc_entries()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_to_string(
    handle: *mut StatLogSinkHandle,
    result: *mut StringDto,
) {
    let s = (*handle).0.to_string();
    (*result) = StringDto::from(s);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_sink_to_object(
    handle: *mut StatLogSinkHandle,
) -> *mut c_void {
    let obj = (*handle).0.to_object();
    match obj {
        Some(obj) => {
            let x = obj.downcast_ref::<FfiPropertyTreeWriter>().unwrap();
            x.handle
        }
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_type_to_string(key: u32, result: *mut *const u8) -> usize {
    let type_str = stat_type_as_str(key).unwrap();
    (*result) = type_str.as_ptr();
    type_str.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_detail_enum_to_string(
    detail: u8,
    result: *mut *const u8,
) -> usize {
    let detail: DetailType = FromPrimitive::from_u8(detail).unwrap();
    let s = detail.as_str();
    (*result) = s.as_ptr();
    s.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_detail_to_string(key: u32, result: *mut *const u8) -> usize {
    let s = stat_detail_as_str(key).unwrap();
    (*result) = s.as_ptr();
    s.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_dir_to_string(key: u32, result: *mut *const u8) -> usize {
    let s = stat_dir_as_str(key).unwrap();
    (*result) = s.as_ptr();
    s.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_stop(handle: *mut StatHandle) {
    (*handle).0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_add(
    handle: *mut StatHandle,
    stat_type: u8,
    detail: u8,
    dir: u8,
    value: u64,
    detail_only: bool,
) {
    let stat_type = FromPrimitive::from_u8(stat_type).unwrap();
    let detail = FromPrimitive::from_u8(detail).unwrap();
    let dir = FromPrimitive::from_u8(dir).unwrap();
    (*handle)
        .0
        .add(stat_type, detail, dir, value, detail_only)
        .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_counters(
    handle: *mut StatHandle,
    sink_handle: *mut StatLogSinkHandle,
) {
    let sink = (*sink_handle).0.as_mut();
    (*handle).0.log_counters(sink).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_log_samples(
    handle: *mut StatHandle,
    sink_handle: *mut StatLogSinkHandle,
) {
    let sink = (*sink_handle).0.as_mut();
    (*handle).0.log_samples(sink).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_define_histogram(
    handle: *mut StatHandle,
    stat_type: u8,
    detail: u8,
    dir: u8,
    intervals: *const u64,
    intervals_len: usize,
    bin_count: u64,
) {
    let stat_type = FromPrimitive::from_u8(stat_type).unwrap();
    let detail = FromPrimitive::from_u8(detail).unwrap();
    let dir = FromPrimitive::from_u8(dir).unwrap();
    let intervals = std::slice::from_raw_parts(intervals, intervals_len);
    (*handle)
        .0
        .define_histogram(stat_type, detail, dir, intervals, bin_count);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_update_histogram(
    handle: *mut StatHandle,
    stat_type: u8,
    detail: u8,
    dir: u8,
    index: u64,
    addend: u64,
) {
    let stat_type = FromPrimitive::from_u8(stat_type).unwrap();
    let detail = FromPrimitive::from_u8(detail).unwrap();
    let dir = FromPrimitive::from_u8(dir).unwrap();
    (*handle)
        .0
        .update_histogram(stat_type, detail, dir, index, addend);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_last_reset_s(handle: *mut StatHandle) -> u64 {
    (*handle).0.last_reset().as_secs()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_clear(handle: *mut StatHandle) {
    (*handle).0.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_configure(
    handle: *mut StatHandle,
    stat_type: u8,
    detail: u8,
    dir: u8,
    interval: usize,
    capacity: usize,
) {
    let stat_type = FromPrimitive::from_u8(stat_type).unwrap();
    let detail = FromPrimitive::from_u8(detail).unwrap();
    let dir = FromPrimitive::from_u8(dir).unwrap();
    (*handle)
        .0
        .configure(stat_type, detail, dir, interval, capacity);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_disable_sampling(
    handle: *mut StatHandle,
    stat_type: u8,
    detail: u8,
    dir: u8,
) {
    let stat_type = FromPrimitive::from_u8(stat_type).unwrap();
    let detail = FromPrimitive::from_u8(detail).unwrap();
    let dir = FromPrimitive::from_u8(dir).unwrap();
    (*handle).0.disable_sampling(stat_type, detail, dir);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_count(
    handle: *mut StatHandle,
    stat_type: u8,
    detail: u8,
    dir: u8,
) -> u64 {
    let stat_type = FromPrimitive::from_u8(stat_type).unwrap();
    let detail = FromPrimitive::from_u8(detail).unwrap();
    let dir = FromPrimitive::from_u8(dir).unwrap();
    (*handle).0.count(stat_type, detail, dir)
}
