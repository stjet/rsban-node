use crate::config::TxnTrackingConfig;

#[repr(C)]
pub struct TxnTrackingConfigDto {
    pub enable: bool,
    pub min_read_txn_time_ms: i64,
    pub min_write_txn_time_ms: i64,
    pub ignore_writes_below_block_processor_max_time: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_txn_tracking_config_create(dto: *mut TxnTrackingConfigDto) {
    let cfg = TxnTrackingConfig::new();
    let dto = &mut (*dto);
    dto.enable = cfg.enable;
    dto.min_read_txn_time_ms = cfg.min_read_txn_time_ms;
    dto.min_write_txn_time_ms = cfg.min_write_txn_time_ms;
    dto.ignore_writes_below_block_processor_max_time =
        cfg.ignore_writes_below_block_processor_max_time;
}
