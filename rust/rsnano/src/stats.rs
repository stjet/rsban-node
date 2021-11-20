pub struct StatConfig {
    /** If true, sampling of counters is enabled */
    pub sampling_enabled: bool,

    /** How many sample intervals to keep in the ring buffer */
    pub capacity: usize,

    /** Sample interval in milliseconds */
    pub interval: usize,

    /** How often to log sample array, in milliseconds. Default is 0 (no logging) */
    pub log_interval_samples: usize,

    /** How often to log counters, in milliseconds. Default is 0 (no logging) */
    pub log_interval_counters: usize,

    /** Maximum number of log outputs before rotating the file */
    pub log_rotation_count: usize,

    /** If true, write headers on each counter or samples writeout. The header contains log type and the current wall time. */
    pub log_headers: bool,

    /** Filename for the counter log  */
    pub log_counters_filename: String,

    /** Filename for the sampling log */
    pub log_samples_filename: String,
}
impl StatConfig {
    pub fn new() -> Self {
        Self {
            sampling_enabled: false,
            capacity: 0,
            interval: 0,
            log_interval_samples: 0,
            log_interval_counters: 0,
            log_rotation_count: 100,
            log_headers: true,
            log_counters_filename: "counters.stat".to_string(),
            log_samples_filename: "samples.stat".to_string(),
        }
    }
}
