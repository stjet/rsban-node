use super::{
    ChannelsViewModel, LedgerStatsViewModel, MessageStatsViewModel, MessageTableViewModel,
    NodeRunnerViewModel, TabBarViewModel,
};
use crate::{
    channels::Channels, ledger_stats::LedgerStats, message_collection::MessageCollection,
    message_recorder::MessageRecorder, node_factory::NodeFactory, node_runner::NodeRunner,
    nullable_runtime::NullableRuntime,
};
use rsnano_node::{block_processing::BlockProcessorInfo, consensus::VoteProcessorInfo,
    cementation::ConfirmingSetInfo, consensus::ActiveElectionsInfo};
use rsnano_nullable_clock::{SteadyClock, Timestamp};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

pub(crate) struct AppViewModel {
    pub msg_recorder: Arc<MessageRecorder>,
    pub node_runner: NodeRunnerViewModel,
    pub message_table: MessageTableViewModel,
    pub tabs: TabBarViewModel,
    ledger_stats: LedgerStats,
    channels: Channels,
    clock: Arc<SteadyClock>,
    last_update: Option<Timestamp>,
    pub aec_info: ActiveElectionsInfo,
    pub confirming_set: ConfirmingSetInfo,
    pub block_processor_info: BlockProcessorInfo,
    pub vote_processor_info: VoteProcessorInfo,
}

impl AppViewModel {
    pub(crate) fn new(runtime: Arc<NullableRuntime>, node_factory: NodeFactory) -> Self {
        let node_runner = NodeRunner::new(runtime, node_factory);
        let messages = Arc::new(RwLock::new(MessageCollection::default()));
        let msg_recorder = Arc::new(MessageRecorder::new(messages.clone()));
        let clock = Arc::new(SteadyClock::default());
        Self {
            node_runner: NodeRunnerViewModel::new(node_runner, msg_recorder.clone(), clock.clone()),
            message_table: MessageTableViewModel::new(messages.clone()),
            tabs: TabBarViewModel::new(),
            msg_recorder,
            channels: Channels::new(messages),
            clock,
            ledger_stats: LedgerStats::new(),
            last_update: None,
            aec_info: Default::default(),
            confirming_set: Default::default(),
            block_processor_info: Default::default(),
            vote_processor_info: Default::default(),
        }
    }

    pub(crate) fn with_runtime(runtime: tokio::runtime::Handle) -> Self {
        Self::new(
            Arc::new(NullableRuntime::new(runtime.clone())),
            NodeFactory::new(runtime),
        )
    }

    pub(crate) fn update(&mut self) {
        let now = self.clock.now();
        if let Some(last_update) = self.last_update {
            if now - last_update < Duration::from_millis(500) {
                return;
            }
        }

        if let Some(node) = self.node_runner.node() {
            self.ledger_stats.update(node, now);
            let channels = node.network_info.read().unwrap().list_realtime_channels(0);
            let telemetries = node.telemetry.get_all_telemetries();
            let (peered_reps, min_rep_weight) = {
                let guard = node.online_reps.lock().unwrap();
                (guard.peered_reps(), guard.minimum_principal_weight())
            };
            let rep_weights = node.ledger.rep_weights.clone();
            self.channels.update(
                channels,
                telemetries,
                peered_reps,
                &rep_weights,
                min_rep_weight,
            );
            self.aec_info = node.active.info();
            self.confirming_set = node.confirming_set.info();
            self.block_processor_info = node.block_processor.info();
            self.vote_processor_info = node.vote_processor_queue.info();
        }

        self.message_table.update_message_counts();

        self.last_update = Some(now);
    }

    pub(crate) fn message_stats(&self) -> MessageStatsViewModel {
        MessageStatsViewModel::new(&self.msg_recorder)
    }

    pub(crate) fn ledger_stats(&self) -> LedgerStatsViewModel {
        LedgerStatsViewModel::new(&self.ledger_stats)
    }

    pub(crate) fn channels(&mut self) -> ChannelsViewModel {
        ChannelsViewModel::new(&mut self.channels)
    }
}

impl Default for AppViewModel {
    fn default() -> Self {
        Self::new(Arc::new(NullableRuntime::default()), NodeFactory::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initial_status() {
        let model = AppViewModel::new(
            Arc::new(NullableRuntime::new_null()),
            NodeFactory::new_null(),
        );
        assert_eq!(model.node_runner.can_start_node(), true);
        assert_eq!(model.node_runner.can_stop_node(), false);
        assert_eq!(model.node_runner.status(), "not running");
        assert_eq!(model.message_stats().send_rate(), "0");
    }

    #[tokio::test]
    async fn starting_node() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());

        model.node_runner.start_node();

        assert_eq!(model.node_runner.can_start_node(), false);
        assert_eq!(model.node_runner.can_stop_node(), false);
        assert_eq!(model.node_runner.status(), "starting...");
        assert_eq!(runtime.blocking_spawns(), 1);
    }

    #[tokio::test]
    async fn starting_completed() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());
        model.node_runner.start_node();

        runtime.run_nulled_blocking_task();

        assert_eq!(model.node_runner.status(), "running");
        assert_eq!(model.node_runner.can_start_node(), false);
        assert_eq!(model.node_runner.can_stop_node(), true);
    }

    #[tokio::test]
    async fn stopping_node() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());
        model.node_runner.start_node();
        runtime.run_nulled_blocking_task();
        model.node_runner.stop_node();
        assert_eq!(model.node_runner.can_start_node(), false);
        assert_eq!(model.node_runner.can_stop_node(), false);
        assert_eq!(model.node_runner.status(), "stopping...");
        assert_eq!(runtime.blocking_spawns(), 2);
    }

    #[tokio::test]
    async fn stopping_completed() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());
        model.node_runner.start_node();
        runtime.run_nulled_blocking_task();
        model.node_runner.stop_node();
        runtime.run_nulled_blocking_task();
        assert_eq!(model.node_runner.can_start_node(), true);
        assert_eq!(model.node_runner.can_stop_node(), false);
        assert_eq!(model.node_runner.status(), "not running");
    }
}
