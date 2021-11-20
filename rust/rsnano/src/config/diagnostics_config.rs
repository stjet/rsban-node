pub struct TxnTrackingConfig {
    /** If true, enable tracking for transaction read/writes held open longer than the min time variables */
    pub enable: bool,
    pub min_read_txn_time_ms: i64,
    pub min_write_txn_time_ms: i64,
    pub ignore_writes_below_block_processor_max_time: bool,
}

impl TxnTrackingConfig {
    pub fn new() -> Self {
        Self {
            enable: false,
            min_read_txn_time_ms: 5000,
            min_write_txn_time_ms: 500,
            ignore_writes_below_block_processor_max_time: true,
        }
    }
}
