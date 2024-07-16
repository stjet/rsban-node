use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{utils::get_cpu_count, work::WorkPoolImpl, Networks};
use rsnano_node::{
    config::{NodeConfig, NodeFlags},
    node::{Node, NodeExt},
    transport::NullSocketObserver,
    utils::AsyncRuntime,
    NetworkParams, DEV_NETWORK_PARAMS,
};
use std::{str::FromStr, sync::Arc, time::Duration};

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct InitializeArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl InitializeArgs {
    pub(crate) fn initialize(&self) {
        let network_params = if let Some(network) = &self.network {
            NetworkParams::new(Networks::from_str(&network).unwrap())
        } else {
            DEV_NETWORK_PARAMS.to_owned()
        };

        let path = get_path(&self.data_path, &self.network);

        std::fs::create_dir_all(&path).unwrap();

        let config = NodeConfig::new(
            Some(network_params.network.default_node_port),
            &network_params,
            get_cpu_count(),
        );

        let flags = NodeFlags::new();
        let async_rt = Arc::new(AsyncRuntime::default());

        let work = Arc::new(WorkPoolImpl::new(
            network_params.work.clone(),
            config.work_threads as usize,
            Duration::from_nanos(config.pow_sleep_interval_ns as u64),
        ));

        let node = Arc::new(Node::new(
            async_rt,
            path,
            config,
            network_params,
            flags,
            work,
            Arc::new(NullSocketObserver::new()),
            Box::new(|_, _, _, _, _, _| {}),
            Box::new(|_, _| {}),
            Box::new(|_, _, _, _| {}),
        ));

        node.start();
        node.stop();
    }
}
