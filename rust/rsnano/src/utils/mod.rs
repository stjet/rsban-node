mod blake2b;
mod buffer;
mod io_context;
mod json;
mod logger_mt;
mod thread_pool;
mod toml;

pub use blake2b::*;
pub use buffer::*;
pub use io_context::*;
pub use json::*;
pub use logger_mt::{Logger, NullLogger};
pub use thread_pool::*;
pub use toml::*;

pub fn seconds_since_epoch() -> u64 {
    chrono::Utc::now().timestamp() as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorCode {
    pub val: i32,
    pub category: u8,
}

pub mod error_category {
    pub const GENERIC: u8 = 0;
    pub const SYSTEM: u8 = 1;
}

impl Default for ErrorCode {
    fn default() -> Self {
        Self {
            val: 0,
            category: error_category::SYSTEM,
        }
    }
}

impl ErrorCode {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_ok(&self) -> bool {
        !self.is_err()
    }

    pub fn is_err(&self) -> bool {
        self.val != 0
    }

    pub fn not_supported() -> Self {
        ErrorCode {
            val: 95,
            category: error_category::GENERIC,
        }
    }

    pub fn no_buffer_space() -> Self {
        ErrorCode {
            val: 105,
            category: error_category::GENERIC,
        }
    }

    pub fn host_unreachable() -> Self {
        ErrorCode {
            val: 113,
            category: error_category::GENERIC,
        }
    }

    pub fn fault() -> Self {
        ErrorCode {
            val: 14,
            category: error_category::GENERIC,
        }
    }
}
