mod blake2b;
mod blocks;
mod buffer;
mod io_context;
mod json;
mod logger_mt;
mod thread_pool;
mod toml;

mod uniquer;
use std::net::Ipv6Addr;

use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};
pub use uniquer::Uniquer;

mod hardened_constants;
pub(crate) use hardened_constants::HardenedConstants;

pub use blake2b::*;
pub use blocks::*;
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

pub fn ip_address_hash_raw(address: &Ipv6Addr, port: u16) -> u64 {
    let address_bytes = address.octets();
    let mut hasher = VarBlake2b::new(8).unwrap();
    hasher.update(&HardenedConstants::get().random_128.to_be_bytes());
    if port != 0 {
        hasher.update(port.to_ne_bytes());
    }
    hasher.update(address_bytes);
    let mut result = 0;
    hasher.finalize_variable(|res| result = u64::from_ne_bytes(res.try_into().unwrap()));
    result
}
