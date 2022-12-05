mod json;
use std::time::SystemTime;

pub use json::*;

mod stream;
pub use stream::*;

mod toml;
pub use toml::*;

mod logger;
pub use logger::{Logger, NullLogger};

pub trait Serialize {
    fn serialized_size() -> usize;
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()>;
}

pub trait Deserialize {
    type Target;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target>;
}

impl Serialize for u64 {
    fn serialized_size() -> usize {
        std::mem::size_of::<u64>()
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_u64_be(*self)
    }
}

impl Deserialize for u64 {
    type Target = Self;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<u64> {
        stream.read_u64_be()
    }
}

impl Serialize for [u8; 64] {
    fn serialized_size() -> usize {
        64
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self)
    }
}

impl Deserialize for [u8; 64] {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let mut buffer = [0; 64];
        stream.read_bytes(&mut buffer, 64)?;
        Ok(buffer)
    }
}

pub fn get_cpu_count() -> usize {
    // Try to read overridden value from environment variable
    let value = std::env::var("NANO_HARDWARE_CONCURRENCY")
        .unwrap_or_else(|_| "0".into())
        .parse::<usize>()
        .unwrap_or_default();

    if value > 0 {
        return value;
    }

    //todo: use std::thread::available_concurrency once it's in stable
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        cpuinfo.match_indices("processor").count()
    } else {
        1
    }
}

pub type MemoryIntensiveInstrumentationCallback = extern "C" fn() -> bool;

pub static mut MEMORY_INTENSIVE_INSTRUMENTATION: Option<MemoryIntensiveInstrumentationCallback> =
    None;
pub static mut IS_SANITIZER_BUILD: Option<MemoryIntensiveInstrumentationCallback> = None;

pub fn memory_intensive_instrumentation() -> bool {
    unsafe {
        match MEMORY_INTENSIVE_INSTRUMENTATION {
            Some(f) => f(),
            None => false,
        }
    }
}

pub fn is_sanitizer_build() -> bool {
    unsafe { IS_SANITIZER_BUILD.expect("IS_SANITIZER_BUILD missing")() }
}

pub fn seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
