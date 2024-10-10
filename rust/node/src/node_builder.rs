use crate::{
    config::{NodeConfig, NodeFlags},
    consensus::{BalanceChangedCallback, ElectionEndCallback, VoteProcessedCallback2},
    transport::PublishedCallback,
    working_path_for, NetworkParams, Node, NodeArgs,
};
use rsnano_core::{utils::get_cpu_count, work::WorkPoolImpl, Networks, Vote, VoteCode, VoteSource};
use rsnano_network::ChannelId;
use std::{path::PathBuf, sync::Arc, time::Duration};

#[derive(Default)]
pub struct NodeCallbacks {
    pub on_election_end: Option<ElectionEndCallback>,
    pub on_balance_changed: Option<BalanceChangedCallback>,
    pub on_vote: Option<VoteProcessedCallback2>,
    pub on_publish: Option<PublishedCallback>,
}

impl NodeCallbacks {
    pub fn builder() -> NodeCallbacksBuilder {
        NodeCallbacksBuilder::new()
    }
}

pub struct NodeCallbacksBuilder(NodeCallbacks);

impl NodeCallbacksBuilder {
    fn new() -> Self {
        Self(NodeCallbacks::default())
    }

    pub fn on_election_end(mut self, callback: ElectionEndCallback) -> Self {
        self.0.on_election_end = Some(callback);
        self
    }

    pub fn on_balance_changed(mut self, callback: BalanceChangedCallback) -> Self {
        self.0.on_balance_changed = Some(callback);
        self
    }

    pub fn on_vote(
        mut self,
        callback: Box<dyn Fn(&Arc<Vote>, ChannelId, VoteSource, VoteCode) + Send + Sync>,
    ) -> Self {
        self.0.on_vote = Some(callback);
        self
    }

    pub fn on_publish(mut self, callback: PublishedCallback) -> Self {
        self.0.on_publish = Some(callback);
        self
    }

    pub fn finish(self) -> NodeCallbacks {
        self.0
    }
}

pub struct NodeBuilder {
    network: Networks,
    is_nulled: bool,
    runtime: Option<tokio::runtime::Handle>,
    data_path: Option<PathBuf>,
    config: Option<NodeConfig>,
    network_params: Option<NetworkParams>,
    flags: Option<NodeFlags>,
    work: Option<Arc<WorkPoolImpl>>,
    callbacks: Option<NodeCallbacks>,
}

impl NodeBuilder {
    pub fn new(network: Networks) -> Self {
        Self::with_nulled(network, false)
    }

    pub fn new_null(network: Networks) -> Self {
        Self::with_nulled(network, true)
    }

    pub fn with_nulled(network: Networks, is_nulled: bool) -> Self {
        Self {
            network,
            is_nulled,
            runtime: None,
            data_path: None,
            config: None,
            network_params: None,
            flags: None,
            work: None,
            callbacks: None,
        }
    }

    pub fn runtime(mut self, runtime: tokio::runtime::Handle) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn data_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_path = Some(path.into());
        self
    }

    pub fn config(mut self, config: NodeConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn network_params(mut self, network_params: NetworkParams) -> Self {
        self.network_params = Some(network_params);
        self
    }

    pub fn flags(mut self, flags: NodeFlags) -> Self {
        self.flags = Some(flags);
        self
    }

    pub fn work(mut self, work: Arc<WorkPoolImpl>) -> Self {
        self.work = Some(work);
        self
    }

    pub fn callbacks(mut self, callbacks: NodeCallbacks) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn finish(self) -> anyhow::Result<Node> {
        let runtime = self
            .runtime
            .unwrap_or_else(|| tokio::runtime::Handle::current());

        let data_path = match self.data_path {
            Some(path) => path,
            None => {
                working_path_for(self.network).ok_or_else(|| anyhow!("working path not found"))?
            }
        };

        let network_params = self
            .network_params
            .unwrap_or_else(|| NetworkParams::new(self.network));

        let config = match self.config {
            Some(c) => c,
            None => {
                let cpu_count = get_cpu_count();
                NodeConfig::new(None, &network_params, cpu_count)
            }
        };

        let flags = self.flags.unwrap_or_default();
        let work = self.work.unwrap_or_else(|| {
            Arc::new(WorkPoolImpl::new(
                network_params.work.clone(),
                config.work_threads as usize,
                Duration::from_nanos(config.pow_sleep_interval_ns as u64),
            ))
        });

        let callbacks = self.callbacks.unwrap_or_default();

        let node = if self.is_nulled {
            Node::new_null_with_callbacks(callbacks)
        } else {
            let args = NodeArgs {
                runtime,
                data_path,
                config,
                network_params,
                flags,
                work,
                callbacks,
            };
            Node::new_with_args(args)
        };
        Ok(node)
    }
}
