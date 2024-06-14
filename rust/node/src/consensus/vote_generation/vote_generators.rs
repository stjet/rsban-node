use crate::{
    config::NodeConfig,
    consensus::VoteProcessorQueue,
    representatives::RepresentativeRegister,
    stats::Stats,
    transport::{ChannelEnum, InboundCallback, Network},
    utils::AsyncRuntime,
    wallets::Wallets,
    NetworkParams,
};

use super::{vote_generator::VoteGenerator, LocalVoteHistory};
use rsnano_core::{utils::ContainerInfoComponent, BlockEnum, BlockHash, PublicKey, Root, Vote};
use rsnano_ledger::Ledger;
use std::{
    net::{Ipv6Addr, SocketAddrV6},
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct VoteGenerators {
    non_final_vote_generator: VoteGenerator,
    final_vote_generator: VoteGenerator,
}

impl VoteGenerators {
    pub(crate) fn new(
        ledger: Arc<Ledger>,
        wallets: Arc<Wallets>,
        history: Arc<LocalVoteHistory>,
        stats: Arc<Stats>,
        representative_register: Arc<Mutex<RepresentativeRegister>>,
        network: Arc<Network>,
        vote_processor_queue: Arc<VoteProcessorQueue>,
        runtime: Arc<AsyncRuntime>,
        node_id: PublicKey,
        inbound: InboundCallback,
        config: &NodeConfig,
        network_params: &NetworkParams,
    ) -> Self {
        let port = network.port();

        let non_final_vote_generator = VoteGenerator::new(
            ledger.clone(),
            wallets.clone(),
            history.clone(),
            false, //none-final
            stats.clone(),
            representative_register.clone(),
            network.clone(),
            vote_processor_queue.clone(),
            network_params.network.clone(),
            runtime.clone(),
            node_id.clone(),
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, port, 0, 0),
            inbound.clone(),
            Duration::from_secs(network_params.voting.delay_s as u64),
            Duration::from_millis(config.vote_generator_delay_ms as u64),
            config.vote_generator_threshold as usize,
        );

        let final_vote_generator = VoteGenerator::new(
            ledger,
            wallets,
            history,
            true, //final
            stats,
            representative_register,
            network,
            vote_processor_queue,
            network_params.network.clone(),
            runtime,
            node_id,
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, port, 0, 0),
            inbound,
            Duration::from_secs(network_params.voting.delay_s as u64),
            Duration::from_millis(config.vote_generator_delay_ms as u64),
            config.vote_generator_threshold as usize,
        );

        Self {
            non_final_vote_generator,
            final_vote_generator,
        }
    }

    pub fn start(&self) {
        self.non_final_vote_generator.start();
        self.final_vote_generator.start();
    }

    pub fn stop(&self) {
        self.non_final_vote_generator.stop();
        self.final_vote_generator.stop();
    }

    pub(crate) fn generate_final_vote(&self, root: &Root, hash: &BlockHash) {
        self.final_vote_generator.add(root, hash);
    }

    pub(crate) fn generate_final_votes(
        &self,
        blocks: &[Arc<BlockEnum>],
        channel: Arc<ChannelEnum>,
    ) -> usize {
        self.final_vote_generator.generate(blocks, channel)
    }

    pub fn generate_non_final_vote(&self, root: &Root, hash: &BlockHash) {
        self.non_final_vote_generator.add(root, hash);
    }

    pub fn generate_non_final_votes(
        &self,
        blocks: &[Arc<BlockEnum>],
        channel: Arc<ChannelEnum>,
    ) -> usize {
        self.non_final_vote_generator.generate(blocks, channel)
    }

    pub fn set_reply_action(
        &self,
        action: Arc<dyn Fn(&Arc<Vote>, &Arc<ChannelEnum>) + Send + Sync>,
    ) {
        self.non_final_vote_generator
            .set_reply_action(action.clone());

        self.final_vote_generator.set_reply_action(action.clone());
    }

    pub(crate) fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                self.non_final_vote_generator
                    .collect_container_info("non_final"),
                self.final_vote_generator.collect_container_info("final"),
            ],
        )
    }
}
