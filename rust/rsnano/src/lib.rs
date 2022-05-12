#![allow(clippy::missing_safety_doc)]
#![allow(dead_code)]

#[macro_use]
extern crate static_assertions;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate anyhow;

mod bandwidth_limiter;
mod blocks;
mod config;
mod epoch;
pub mod ffi;
mod ipc;
mod logger_mt;
mod numbers;
mod secure;
mod signatures;
mod state_block_signature_verification;
mod stats;
mod token_bucket;
mod uniquer;
mod utils;
mod vote;

pub use bandwidth_limiter::*;
pub use blocks::*;
pub use config::*;
pub use epoch::*;
pub use ipc::*;
pub(crate) use logger_mt::*;
pub use numbers::*;
pub use secure::*;
pub use signatures::*;
pub(crate) use state_block_signature_verification::*;
pub use stats::*;
pub use token_bucket::*;
pub(crate) use uniquer::*;
pub use utils::*;
pub(crate) use vote::*;

pub trait FullHash {
    fn full_hash(&self) -> BlockHash;
}
