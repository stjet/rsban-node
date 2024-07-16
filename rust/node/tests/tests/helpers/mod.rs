use rsnano_core::{work::WorkPoolImpl, Amount, Networks};
use rsnano_node::{
    config::{NodeConfig, NodeFlags},
    node::Node,
    transport::NullSocketObserver,
    unique_path,
    utils::AsyncRuntime,
    NetworkParams,
};
use std::{net::TcpListener, sync::Arc, time::Duration};

pub(crate) struct System {
    runtime: Arc<AsyncRuntime>,
    network_params: NetworkParams,
    work: Arc<WorkPoolImpl>,
}

impl System {
    pub(crate) fn new() -> Self {
        let network_params = NetworkParams::new(Networks::NanoDevNetwork);

        Self {
            runtime: Arc::new(AsyncRuntime::default()),
            work: Arc::new(WorkPoolImpl::new(
                network_params.work.clone(),
                1,
                Duration::ZERO,
            )),
            network_params,
        }
    }

    pub(crate) fn default_config() -> NodeConfig {
        let network_params = NetworkParams::new(Networks::NanoDevNetwork);
        let port = get_available_port();
        let mut config = NodeConfig::new(Some(port), &network_params, 1);
        config.representative_vote_weight_minimum = Amount::zero();
        config
    }

    pub(crate) fn build_node<'a>(&'a mut self) -> NodeBuilder<'a> {
        NodeBuilder {
            system: self,
            config: None,
        }
    }

    fn create_node(&mut self, config: NodeConfig) -> Node {
        let path = unique_path().expect("Could not get a unique path");
        let flags = NodeFlags::default();
        let node = Node::new(
            self.runtime.clone(),
            path,
            config,
            self.network_params.clone(),
            flags,
            self.work.clone(),
            Arc::new(NullSocketObserver::new()),
            Box::new(|_, _, _, _, _, _| {}),
            Box::new(|_, _| {}),
            Box::new(|_, _, _, _| {}),
        );

        node
    }
}

pub(crate) struct NodeBuilder<'a> {
    system: &'a mut System,
    config: Option<NodeConfig>,
}

impl<'a> NodeBuilder<'a> {
    pub(crate) fn config(mut self, cfg: NodeConfig) -> Self {
        self.config = Some(cfg);
        self
    }

    pub(crate) fn finish(self) -> Node {
        let config = self.config.unwrap_or_else(|| System::default_config());
        self.system.create_node(config)
    }
}

fn get_available_port() -> u16 {
    (1025..65535)
        .find(|port| is_port_available(*port))
        .expect("Could not find an available port")
}

fn is_port_available(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => true,
        Err(_) => false,
    }
}
