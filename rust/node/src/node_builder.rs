use crate::{
    config::{NodeConfig, NodeFlags},
    consensus::{AccountBalanceChangedCallback, ElectionEndCallback},
    transport::PublishedCallback,
    NetworkParams, Node, NodeArgs,
};
use rsnano_core::{work::WorkPoolImpl, Networks, Vote, VoteCode, VoteSource};
use rsnano_network::ChannelId;
use std::{path::PathBuf, sync::Arc};

pub struct NodeBuilder {
    network: Networks,
    runtime: Option<tokio::runtime::Handle>,
    data_path: Option<PathBuf>,
    config: Option<NodeConfig>,
    network_params: Option<NetworkParams>,
    flags: Option<NodeFlags>,
    work: Option<Arc<WorkPoolImpl>>,
    on_election_end: Option<ElectionEndCallback>,
    on_balance_changed: Option<AccountBalanceChangedCallback>,
    on_vote: Option<Box<dyn Fn(&Arc<Vote>, ChannelId, VoteSource, VoteCode) + Send + Sync>>,
    on_publish: Option<PublishedCallback>,
}

impl NodeBuilder {
    pub fn new(network: Networks) -> Self {
        Self {
            network,
            runtime: None,
            data_path: None,
            config: None,
            network_params: None,
            flags: None,
            work: None,
            on_vote: None,
            on_publish: None,
            on_election_end: None,
            on_balance_changed: None,
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

    pub fn on_election_end(mut self, callback: ElectionEndCallback) -> Self {
        self.on_election_end = Some(callback);
        self
    }

    pub fn on_balance_changed(mut self, callback: AccountBalanceChangedCallback) -> Self {
        self.on_balance_changed = Some(callback);
        self
    }

    pub fn on_vote(
        mut self,
        callback: Box<dyn Fn(&Arc<Vote>, ChannelId, VoteSource, VoteCode) + Send + Sync>,
    ) -> Self {
        self.on_vote = Some(callback);
        self
    }

    pub fn on_publish(mut self, callback: PublishedCallback) -> Self {
        self.on_publish = Some(callback);
        self
    }

    pub fn finish(self) -> Node {
        let runtime = self
            .runtime
            .unwrap_or_else(|| tokio::runtime::Handle::current());

        let data_path = self.data_path.unwrap_or_else(|| unimplemented!());
        let config = self.config.unwrap_or_else(|| unimplemented!());
        let network_params = self.network_params.unwrap_or_else(|| unimplemented!());
        let flags = self.flags.unwrap_or_default();
        let work = self.work.unwrap_or_else(|| unimplemented!());

        let on_election_end = self
            .on_election_end
            .unwrap_or_else(|| Box::new(|_, _, _, _, _, _| {}));

        let on_balance_changed = self
            .on_balance_changed
            .unwrap_or_else(|| Box::new(|_, _| {}));

        let on_vote = self.on_vote.unwrap_or_else(|| Box::new(|_, _, _, _| {}));

        let args = NodeArgs {
            runtime,
            data_path,
            config,
            network_params,
            flags,
            work,
            on_election_end,
            on_balance_changed,
            on_vote,
            on_publish: self.on_publish,
        };
        Node::new(args)
    }
}
