mod blake2b;
mod json;
mod stream;
mod thread_pool;
mod toml;

pub use blake2b::*;
pub use json::*;
pub use stream::*;
pub use thread_pool::*;
pub use toml::*;

pub fn seconds_since_epoch() -> u64 {
    chrono::Utc::now().timestamp() as u64
}

pub fn get_cpu_count() -> usize {
    //todo: use std::thread::available_concurrency once it's in stable
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        cpuinfo.match_indices("processor").count()
    } else {
        1
    }
}
