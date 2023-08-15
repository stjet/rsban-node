use std::sync::{Arc, Mutex};

use rsnano_core::{utils::Logger, work::WorkThresholds};
use rsnano_ledger::Ledger;

use crate::{
    block_processing::BlockProcessor, config::NodeFlags, stats::Stats, transport::TcpServer,
    utils::ThreadPool,
};

use super::BootstrapInitiator;

pub struct BootstrapMessageVisitorImpl {
    pub ledger: Arc<Ledger>,
    pub logger: Arc<dyn Logger>,
    pub connection: Arc<TcpServer>,
    pub thread_pool: Arc<dyn ThreadPool>,
    pub block_processor: Arc<BlockProcessor>,
    pub bootstrap_initiator: Arc<BootstrapInitiator>,
    pub stats: Arc<Stats>,
    pub work_thresholds: WorkThresholds,
    pub flags: Arc<Mutex<NodeFlags>>,
    pub processed: bool,
}
