use crate::cli::{get_path, init_tracing};
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::utils::get_cpu_count;
use rsnano_node::{
    config::{
        get_node_toml_config_path, get_rpc_toml_config_path, DaemonConfig, DaemonToml,
        NetworkConstants, NodeFlags,
    },
    NetworkParams, NodeBuilder, NodeExt,
};
use rsnano_rpc_server::{run_rpc_server, RpcServerConfig, RpcServerToml};
use std::{
    fs::read_to_string,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use tokio::net::TcpListener;
use toml::from_str;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct RunDaemonArgs {
    /// Uses the supplied path as the data directory
    #[arg(long, group = "input")]
    data_path: Option<String>,
    /// Uses the supplied network (live, test, beta or dev)
    #[arg(long, group = "input")]
    network: Option<String>,
    /// Pass node configuration values
    /// This takes precedence over any values in the configuration file
    /// This option can be repeated multiple times
    #[arg(long, verbatim_doc_comment)]
    config_overrides: Option<Vec<String>>,
    /// Pass RPC configuration values
    /// This takes precedence over any values in the configuration file
    /// This option can be repeated multiple times.
    #[arg(long, verbatim_doc_comment)]
    rpc_config_overrides: Option<Vec<String>>,
    /// Disables activate_successors in active_elections
    #[arg(long)]
    disable_activate_successors: bool,
    /// Turn off automatic wallet backup process
    #[arg(long)]
    disable_backup: bool,
    /// Turn off use of lazy bootstrap
    #[arg(long)]
    disable_lazy_bootstrap: bool,
    /// Turn off use of legacy bootstrap
    #[arg(long)]
    disable_legacy_bootstrap: bool,
    /// Turn off use of wallet-based bootstrap
    #[arg(long)]
    disable_wallet_bootstrap: bool,
    /// Turn off listener on the bootstrap network so incoming TCP (bootstrap) connections are rejected
    /// Note: this does not impact TCP traffic for the live network.
    #[arg(long, verbatim_doc_comment)]
    disable_bootstrap_listener: bool,
    /// Disables the legacy bulk pull server for bootstrap operations
    #[arg(long)]
    disable_bootstrap_bulk_pull_server: bool,
    /// Disables the legacy bulk push client for bootstrap operations
    #[arg(long)]
    disable_bootstrap_bulk_push_client: bool,
    /// Turn off the ability for ongoing bootstraps to occur
    #[arg(long)]
    disable_ongoing_bootstrap: bool,
    /// Disable ascending bootstrap
    #[arg(long)]
    disable_ascending_bootstrap: bool,
    /// Turn off the request loop
    #[arg(long)]
    disable_request_loop: bool,
    /// Turn off the rep crawler process
    #[arg(long)]
    disable_rep_crawler: bool,
    /// Turn off use of TCP live network (TCP for bootstrap will remain available)
    #[arg(long)]
    disable_tcp_realtime: bool,
    /// Do not provide any telemetry data to nodes requesting it. Responses are still made to requests, but they will have an empty payload.
    #[arg(long)]
    disable_providing_telemetry_metrics: bool,
    /// Disable deletion of unchecked blocks after processing.
    #[arg(long)]
    disable_block_processor_unchecked_deletion: bool,
    /// Disables block republishing by disabling the local_block_broadcaster component
    #[arg(long)]
    disable_block_processor_republishing: bool,
    /// Allow multiple connections to the same peer in bootstrap attempts
    #[arg(long)]
    allow_bootstrap_peers_duplicates: bool,
    /// Enable experimental ledger pruning
    #[arg(long)]
    enable_pruning: bool,
    /// Increase bootstrap processor limits to allow more blocks before hitting full state and verify/write more per database call. Also disable deletion of processed unchecked blocks.
    #[arg(long)]
    fast_bootstrap: bool,
    /// Increase block processor transaction batch write size, default 0 (limited by config block_processor_batch_max_time), 256k for fast_bootstrap
    #[arg(long)]
    block_processor_batch_size: Option<usize>,
    /// Increase block processor allowed blocks queue size before dropping live network packets and holding bootstrap download, default 65536, 1 million for fast_bootstrap
    #[arg(long)]
    block_processor_full_size: Option<usize>,
    /// Increase batch signature verification size in block processor, default 0 (limited by config signature_checker_threads), unlimited for fast_bootstrap
    #[arg(long)]
    block_processor_verification_size: Option<usize>,
    /// Vote processor queue size before dropping votes, default 144k
    #[arg(long)]
    vote_processor_capacity: Option<usize>,
}

impl RunDaemonArgs {
    pub(crate) async fn run_daemon(&self) -> Result<()> {
        let dirs = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or(String::from("info"));

        init_tracing(dirs);

        let path = get_path(&self.data_path, &self.network);
        let network = NetworkConstants::active_network();
        let network_params = NetworkParams::new(network);
        let parallelism = get_cpu_count();

        std::fs::create_dir_all(&path).map_err(|e| anyhow!("Create dir failed: {:?}", e))?;

        let node_toml_config_path = get_node_toml_config_path(&path);

        let mut daemon_config = DaemonConfig::new(&network_params, parallelism);
        if node_toml_config_path.exists() {
            let daemon_toml_str = read_to_string(node_toml_config_path)?;
            let daemon_toml: DaemonToml = from_str(&daemon_toml_str)?;
            daemon_config.merge_toml(&daemon_toml);
        }

        let rpc_toml_config_path = get_rpc_toml_config_path(&path);

        let mut rpc_server_config = RpcServerConfig::default_for(network, parallelism);
        if rpc_toml_config_path.exists() {
            let rpc_server_toml_str = read_to_string(rpc_toml_config_path)?;
            let rpc_server_toml: RpcServerToml = from_str(&rpc_server_toml_str)?;
            rpc_server_config.merge_toml(&rpc_server_toml);
        }

        let mut flags = NodeFlags::new();
        self.set_flags(&mut flags);

        let node = NodeBuilder::new(network_params.network.current_network)
            .data_path(path)
            .network_params(network_params)
            .flags(flags)
            .finish()
            .unwrap();

        let node = Arc::new(node);
        node.start();

        let (tx_stop, rx_stop) = tokio::sync::oneshot::channel();

        if daemon_config.rpc_enable {
            let ip_addr = IpAddr::from_str(&rpc_server_config.address)?;
            let socket_addr = SocketAddr::new(ip_addr, rpc_server_config.port);
            let listener = TcpListener::bind(socket_addr).await?;
            run_rpc_server(
                node.clone(),
                listener,
                rpc_server_config.enable_control,
                tx_stop,
                shutdown_signal(rx_stop),
            )
            .await?;
        } else {
            shutdown_signal(rx_stop).await;
        };

        node.stop();
        Ok(())
    }

    pub(crate) fn set_flags(&self, node_flags: &mut NodeFlags) {
        if let Some(config_overrides) = &self.config_overrides {
            node_flags.config_overrides = config_overrides.clone();
        }
        if let Some(rpc_config_overrides) = &self.rpc_config_overrides {
            node_flags.rpc_config_overrides = rpc_config_overrides.clone();
        }
        node_flags.disable_activate_successors = self.disable_activate_successors;
        node_flags.disable_backup = self.disable_backup;
        node_flags.disable_lazy_bootstrap = self.disable_lazy_bootstrap;
        node_flags.disable_legacy_bootstrap = self.disable_legacy_bootstrap;
        node_flags.disable_wallet_bootstrap = self.disable_wallet_bootstrap;
        node_flags.disable_bootstrap_listener = self.disable_bootstrap_listener;
        node_flags.disable_bootstrap_bulk_pull_server = self.disable_bootstrap_bulk_pull_server;
        node_flags.disable_bootstrap_bulk_push_client = self.disable_bootstrap_bulk_push_client;
        node_flags.disable_ongoing_bootstrap = self.disable_ongoing_bootstrap;
        node_flags.disable_ascending_bootstrap = self.disable_ascending_bootstrap;
        node_flags.disable_rep_crawler = self.disable_rep_crawler;
        node_flags.disable_request_loop = self.disable_request_loop;
        node_flags.disable_tcp_realtime = self.disable_tcp_realtime;
        node_flags.disable_providing_telemetry_metrics = self.disable_providing_telemetry_metrics;
        node_flags.disable_block_processor_unchecked_deletion =
            self.disable_block_processor_unchecked_deletion;
        node_flags.disable_block_processor_republishing = self.disable_block_processor_republishing;
        node_flags.allow_bootstrap_peers_duplicates = self.allow_bootstrap_peers_duplicates;
        node_flags.enable_pruning = self.enable_pruning;
        node_flags.fast_bootstrap = self.fast_bootstrap;
        if let Some(block_processor_batch_size) = self.block_processor_batch_size {
            node_flags.block_processor_batch_size = block_processor_batch_size;
        }
        if let Some(block_processor_full_size) = self.block_processor_full_size {
            node_flags.block_processor_full_size = block_processor_full_size;
        }
        if let Some(block_processor_verification_size) = self.block_processor_verification_size {
            node_flags.block_processor_verification_size = block_processor_verification_size;
        }
        if let Some(vote_processor_capacity) = self.vote_processor_capacity {
            node_flags.vote_processor_capacity = vote_processor_capacity;
        }
    }
}

async fn shutdown_signal(tx_stop: tokio::sync::oneshot::Receiver<()>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = tx_stop => {},
    }
}
