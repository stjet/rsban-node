#![allow(clippy::missing_safety_doc)]
#![allow(dead_code)]

#[macro_use]
extern crate static_assertions;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate anyhow;

mod block_arrival;
mod block_processor;
mod blocks;
mod bootstrap;
mod config;
pub mod core;
pub mod datastore;
mod epoch;
pub mod ffi;
mod hardened_constants;
mod ipc;
mod logger_mt;
pub mod messages;
mod numbers;
mod secure;
mod signatures;
mod state_block_signature_verification;
pub mod stats;
pub mod transport;
mod unchecked_info;
mod uniquer;
pub mod utils;
mod voting;
pub mod wallet;
mod websocket;

use crate::core::BlockHash;
pub(crate) use block_arrival::*;
pub(crate) use block_processor::*;
pub use blocks::*;
pub(crate) use bootstrap::*;
pub use config::*;
pub use epoch::*;
pub(crate) use hardened_constants::*;
pub use ipc::*;
pub(crate) use logger_mt::*;
pub use numbers::*;
pub use secure::*;
pub use signatures::*;
pub(crate) use state_block_signature_verification::*;
pub(crate) use unchecked_info::*;
pub(crate) use uniquer::*;
pub(crate) use voting::*;
pub(crate) use websocket::*;

pub trait FullHash {
    fn full_hash(&self) -> BlockHash;
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
