#![allow(clippy::missing_safety_doc)]
#![allow(dead_code)]

#[macro_use]
extern crate static_assertions;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate anyhow;
extern crate core;

pub mod block_processing;
pub mod bootstrap;
pub mod cementing;
pub mod config;
mod ipc;
pub mod messages;

pub mod online_reps;
pub use online_reps::{OnlineReps, OnlineWeightSampler, ONLINE_WEIGHT_QUORUM};
pub(crate) mod online_reps_container;
pub(crate) use online_reps_container::OnlineRepsContainer;

mod secure;
pub mod signatures;
pub mod stats;
pub mod transport;
pub mod utils;
pub mod voting;
pub mod websocket;

pub use ipc::*;
pub use secure::*;
