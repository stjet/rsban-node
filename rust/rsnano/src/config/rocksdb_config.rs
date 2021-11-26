use crate::utils::{get_cpu_count, TomlWriter};
use anyhow::Result;

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

    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> Result<()> {
        toml.put_bool(
            "enable",
            self.enable,
            "Whether to use the RocksDB backend for the ledger database.\ntype:bool",
        )?;
        toml.put_u16("memory_multiplier", self.memory_multiplier as u16, "This will modify how much memory is used represented by 1 (low), 2 (medium), 3 (high). Default is 2.\ntype:uint8")?;
        toml.put_u32("io_threads", self.io_threads, "Number of threads to use with the background compaction and flushing. Number of hardware threads is recommended.\ntype:uint32")?;
        Ok(())
    }
}
