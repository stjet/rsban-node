use rsnano_core::utils::get_cpu_count;
use rsnano_node::{
    config::{DaemonConfig, Networks, NodeFlags},
    Node, NodeBuilder, NodeCallbacks, NodeExt,
};
use rsnano_rpc_server::{run_rpc_server, RpcServerConfig};
use std::{future::Future, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;

pub struct DaemonBuilder {
    network: Networks,
    node_builder: NodeBuilder,
    node_started: Option<Box<dyn FnMut(Arc<Node>) + Send>>,
}

impl DaemonBuilder {
    pub fn new(network: Networks) -> Self {
        Self {
            network,
            node_builder: NodeBuilder::new(network),
            node_started: None,
        }
    }

    pub fn data_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.node_builder = self.node_builder.data_path(path);
        self
    }

    pub fn flags(mut self, flags: NodeFlags) -> Self {
        self.node_builder = self.node_builder.flags(flags);
        self
    }

    pub fn callbacks(mut self, callbacks: NodeCallbacks) -> Self {
        self.node_builder = self.node_builder.callbacks(callbacks);
        self
    }

    pub fn on_node_started(mut self, callback: impl FnMut(Arc<Node>) + Send + 'static) -> Self {
        self.node_started = Some(Box::new(callback));
        self
    }

    pub async fn run<F>(self, shutdown: F) -> anyhow::Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // build node
        let data_path = self.node_builder.get_data_path()?;
        let parallelism = get_cpu_count();
        let daemon_config =
            DaemonConfig::load_from_data_path(self.network, parallelism, &data_path)?;
        let rpc_config =
            RpcServerConfig::load_from_data_path(self.network, parallelism, &data_path)?;
        let node = self.node_builder.finish()?;
        let node = Arc::new(node);

        // start node
        node.start();
        if let Some(mut started_callback) = self.node_started {
            started_callback(node.clone());
        }
        let (tx_stop, rx_stop) = tokio::sync::oneshot::channel();
        let wait_for_shutdown = async move {
            tokio::select! {
                _ = rx_stop =>{}
                _ = shutdown => {}
            }
        };
        if daemon_config.rpc_enable {
            let socket_addr = rpc_config.listening_addr()?;
            let listener = TcpListener::bind(socket_addr).await?;
            run_rpc_server(
                node.clone(),
                listener,
                rpc_config.enable_control,
                tx_stop,
                wait_for_shutdown,
            )
            .await?;
        } else {
            wait_for_shutdown.await;
        };

        node.stop();
        Ok(())
    }
}