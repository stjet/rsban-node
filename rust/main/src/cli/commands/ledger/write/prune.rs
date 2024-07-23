use crate::cli::get_path;
use anyhow::Result;
use clap::Parser;
use rsnano_core::{utils::get_cpu_count, work::WorkPoolImpl};
use rsnano_node::{
    config::{NetworkConstants, NodeConfig, NodeFlags},
    node::Node,
    transport::NullSocketObserver,
    utils::AsyncRuntime,
    NetworkParams,
};
use std::{sync::Arc, time::Duration};

#[derive(Parser)]
pub(crate) struct PruneArgs {
    /// Uses the supplied path as the data directory
    #[arg(long, group = "input")]
    data_path: Option<String>,
    /// Uses the supplied network (live, test, beta or dev)
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl PruneArgs {
    pub(crate) fn prune(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network);

        let network_params = NetworkParams::new(NetworkConstants::active_network());

        let config = NodeConfig::new(
            Some(network_params.network.default_node_port),
            &network_params,
            get_cpu_count(),
        );

        let node_flags = NodeFlags::new();

        let async_rt = Arc::new(AsyncRuntime::default());

        let work = Arc::new(WorkPoolImpl::new(
            network_params.work.clone(),
            config.work_threads as usize,
            Duration::from_nanos(config.pow_sleep_interval_ns as u64),
        ));

        let batch_size = if node_flags.block_processor_batch_size != 0 {
            node_flags.block_processor_batch_size as u64
        } else {
            16 * 1024
        };

        let node = Arc::new(Node::new(
            async_rt,
            path,
            config,
            network_params,
            node_flags,
            work,
            Arc::new(NullSocketObserver::new()),
            Box::new(|_, _, _, _, _, _| {}),
            Box::new(|_, _| {}),
            Box::new(|_, _, _, _| {}),
        ));

        node.ledger_pruning(batch_size, true);

        Ok(())
    }
}
