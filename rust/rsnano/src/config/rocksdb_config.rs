use crate::utils::get_cpu_count;

pub struct RocksDbConfig {
    pub enable: bool,
    pub memory_multiplier: u8,
    pub io_threads: u32,
}

impl RocksDbConfig {
    pub fn using_rocksdb_in_tests() -> bool {
        if let Ok(value) = std::env::var("TEST_USE_ROCKSDB") {
            if let Ok(value) = value.parse::<i32>() {
                return value == 1;
            }
        }

        false
    }

    pub fn new() -> Self {
        Self {
            enable: Self::using_rocksdb_in_tests(),
            memory_multiplier: 2,
            io_threads: get_cpu_count() as u32,
        }
    }
}
