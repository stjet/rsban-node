use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use rsnano_core::{utils::get_cpu_count, work::WorkPoolImpl, Networks};
use rsnano_node::{
    config::{NodeConfig, NodeFlags},
    node::{Node, NodeExt},
    transport::NullSocketObserver,
    utils::AsyncRuntime,
    working_path_for, NetworkParams,
};
use tracing_subscriber::EnvFilter;

fn main() {
    let dirs = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or(String::from(
        "rsnano_ffi=debug,rsnano_node=debug,rsnano_messages=debug,rsnano_ledger=debug,rsnano_store_lmdb=debug,rsnano_core=debug",
    ));
    init_tracing(dirs);
    // TODO set file descriptors limit
    let network = Networks::NanoBetaNetwork;
    let working_path = working_path_for(network).unwrap();
    std::fs::create_dir_all(&working_path).unwrap();

    let network_params = NetworkParams::new(network);
    let config = NodeConfig::new(
        Some(network_params.network.default_node_port),
        &network_params,
        get_cpu_count(),
    );
    let flags = NodeFlags::default();
    let async_rt = Arc::new(AsyncRuntime::default());
    let work = Arc::new(WorkPoolImpl::new(
        network_params.work.clone(),
        config.work_threads as usize,
        Duration::from_nanos(config.pow_sleep_interval_ns as u64),
    ));

    let node = Arc::new(Node::new(
        async_rt,
        working_path,
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

    let finished = Arc::new((Mutex::new(false), Condvar::new()));
    let finished_clone = finished.clone();
    ctrlc::set_handler(move || {
        node.stop();
        *finished_clone.0.lock().unwrap() = true;
        finished_clone.1.notify_all();
    })
    .expect("Error setting Ctrl-C handler");
    let guard = finished.0.lock().unwrap();
    drop(finished.1.wait_while(guard, |g| !*g).unwrap());
}

fn init_tracing(dirs: impl AsRef<str>) {
    let filter = EnvFilter::builder().parse_lossy(dirs);
    let value = std::env::var("NANO_LOG");
    let log_style = value.as_ref().map(|i| i.as_str()).unwrap_or_default();
    match log_style {
        "json" => {
            tracing_subscriber::fmt::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }
        "noansi" => {
            tracing_subscriber::fmt::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .init();
        }
        _ => {
            tracing_subscriber::fmt::fmt()
                .with_env_filter(filter)
                .with_ansi(true)
                .init();
        }
    }
    tracing::debug!(log_style, ?value, "init tracing");
}
