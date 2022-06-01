use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use num::FromPrimitive;

use crate::{
    stat_detail_as_str, stat_type_as_str, DetailType, FileWriter, JsonWriter, Stat, StatConfig,
    StatDatapoint, StatEntry, StatHistogram, StatLogSink,
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

pub struct StatHistogramHandle(StatHistogram);

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_histogram_create(
    intervals: *const u64,
    intervals_len: usize,
    bin_count: u64,
) -> *mut StatHistogramHandle {
    let intervals = std::slice::from_raw_parts(intervals, intervals_len);
    Box::into_raw(Box::new(StatHistogramHandle(StatHistogram::new(
        intervals, bin_count,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_histogram_clone(
    handle: *const StatHistogramHandle,
) -> *mut StatHistogramHandle {
    Box::into_raw(Box::new(StatHistogramHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_histogram_destroy(handle: *mut StatHistogramHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_histogram_add(
    handle: *mut StatHistogramHandle,
    index: u64,
    addend: u64,
) {
    (*handle).0.add(index, addend);
}

pub struct HistogramsBinHandle(Vec<HistogramBinDto>);

#[repr(C)]
pub struct HistogramBinsDto {
    bins: *const HistogramBinDto,
    len: usize,
    handle: *mut HistogramsBinHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_histogram_get_bins(
    handle: *const StatHistogramHandle,
    result: *mut HistogramBinsDto,
) {
    let bins = Box::new(HistogramsBinHandle(
        (*handle)
            .0
            .get_bins()
            .iter()
            .map(|b| HistogramBinDto {
                start_inclusive: b.start_inclusive,
                end_exclusive: b.end_exclusive,
                value: b.value,
                timestamp_ms: b.timestamp.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            })
            .collect(),
    ));
    (*result).bins = bins.0.as_ptr();
    (*result).len = bins.0.len();
    (*result).handle = Box::into_raw(bins);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_histogram_bins_destroy(handle: *mut HistogramsBinHandle) {
    drop(Box::from_raw(handle));
}

pub struct StatEntryHandle(StatEntry);

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_create(
    capacity: usize,
    interval: usize,
) -> *mut StatEntryHandle {
    Box::into_raw(Box::new(StatEntryHandle(StatEntry::new(
        capacity, interval,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_destroy(handle: *mut StatEntryHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_sample_interval(
    handle: *const StatEntryHandle,
) -> usize {
    (*handle).0.sample_interval
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_set_sample_interval(
    handle: *mut StatEntryHandle,
    interval: usize,
) {
    (*handle).0.sample_interval = interval;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_sample_current(
    handle: *const StatEntryHandle,
) -> *mut StatDatapointHandle {
    Box::into_raw(Box::new(StatDatapointHandle(
        (*handle).0.sample_current.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_sample_current_add(
    handle: *mut StatEntryHandle,
    value: u64,
    update_timestamp: bool,
) {
    (*handle).0.sample_current.add(value, update_timestamp);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_sample_current_set_value(
    handle: *mut StatEntryHandle,
    value: u64,
) {
    (*handle).0.sample_current.set_value(value);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_sample_current_set_timestamp(
    handle: *mut StatEntryHandle,
    timestamp_ms: u64,
) {
    (*handle)
        .0
        .sample_current
        .set_timestamp(SystemTime::UNIX_EPOCH + Duration::from_millis(timestamp_ms));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_sample_count(handle: *const StatEntryHandle) -> usize {
    match &(*handle).0.samples {
        Some(s) => s.len(),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_sample(
    handle: *const StatEntryHandle,
    index: usize,
) -> *mut StatDatapointHandle {
    Box::into_raw(Box::new(StatDatapointHandle(
        (*handle).0.samples.as_ref().unwrap()[index].clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_add_sample(
    handle: *mut StatEntryHandle,
    sample: *const StatDatapointHandle,
) {
    if let Some(s) = &mut (*handle).0.samples {
        s.push_back((*sample).0.clone());
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_counter_value(handle: *const StatEntryHandle) -> u64 {
    (*handle).0.counter.get_value()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_counter_timestamp(
    handle: *const StatEntryHandle,
) -> u64 {
    (*handle)
        .0
        .counter
        .get_timestamp()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_counter_add(
    handle: *mut StatEntryHandle,
    addend: u64,
    update_timestamp: bool,
) {
    (*handle).0.counter.add(addend, update_timestamp)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_sample_start_time(
    handle: *const StatEntryHandle,
) -> u64 {
    (*handle)
        .0
        .sample_start_time
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_set_sample_start_time(
    handle: *mut StatEntryHandle,
    time_ms: u64,
) {
    (*handle).0.sample_start_time = UNIX_EPOCH + Duration::from_millis(time_ms);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_define_histogram(
    handle: *mut StatEntryHandle,
    intervals: *const u64,
    intervals_len: usize,
    bin_count: u64,
) {
    let intervals = std::slice::from_raw_parts(intervals, intervals_len);
    (*handle).0.histogram = Some(StatHistogram::new(intervals, bin_count));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_update_histogram(
    handle: *mut StatEntryHandle,
    index: u64,
    addend: u64,
) {
    match &mut (*handle).0.histogram {
        Some(h) => h.add(index, addend),
        None => {}
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_stat_entry_get_histogram(
    handle: *mut StatEntryHandle,
) -> *mut StatHistogramHandle {
    match &mut (*handle).0.histogram {
        Some(h) => Box::into_raw(Box::new(StatHistogramHandle(h.clone()))),
        None => std::ptr::null_mut(),
    }
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
pub unsafe extern "C" fn rsn_stat_log_sink_write_entry(
    handle: *mut StatLogSinkHandle,
    time_ms: u64,
    entry_type: *const c_char,
    detail: *const c_char,
    dir: *const c_char,
    value: u64,
    histogram: *const StatHistogramHandle,
) {
    let wall_time = UNIX_EPOCH + Duration::from_millis(time_ms);
    let entry_type = CStr::from_ptr(entry_type).to_string_lossy();
    let detail = CStr::from_ptr(detail).to_string_lossy();
    let dir = CStr::from_ptr(dir).to_string_lossy();
    let histogram = if histogram.is_null() {
        None
    } else {
        Some(&(*histogram).0)
    };
    (*handle)
        .0
        .write_entry(wall_time, &entry_type, &detail, &dir, value, histogram)
        .unwrap();
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
