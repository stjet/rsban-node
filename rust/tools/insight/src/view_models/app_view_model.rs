use super::{
    LedgerStatsViewModel, MessageStatsViewModel, MessageTableViewModel, NodeRunnerViewModel,
};
use crate::{
    ledger_stats::LedgerStats, message_recorder::MessageRecorder, node_factory::NodeFactory,
    node_runner::NodeRunner, nullable_runtime::NullableRuntime,
};
use std::sync::Arc;

pub(crate) struct AppViewModel {
    pub msg_recorder: Arc<MessageRecorder>,
    pub node_runner: NodeRunnerViewModel,
    pub message_table: MessageTableViewModel,
    ledger_stats: LedgerStats,
}

impl AppViewModel {
    pub(crate) fn new(runtime: Arc<NullableRuntime>, node_factory: NodeFactory) -> Self {
        let node_runner = NodeRunner::new(runtime, node_factory);
        let msg_recorder = Arc::new(MessageRecorder::new());
        Self {
            node_runner: NodeRunnerViewModel::new(node_runner, msg_recorder.clone()),
            message_table: MessageTableViewModel::new(msg_recorder.clone()),
            msg_recorder,
            ledger_stats: LedgerStats::new(),
        }
    }

    pub(crate) fn with_runtime(runtime: tokio::runtime::Handle) -> Self {
        Self::new(
            Arc::new(NullableRuntime::new(runtime.clone())),
            NodeFactory::new(runtime),
        )
    }

    pub(crate) fn update(&mut self) {
        if let Some(node) = self.node_runner.node() {
            self.ledger_stats.update(node);
        }
    }

    pub(crate) fn message_stats(&self) -> MessageStatsViewModel {
        MessageStatsViewModel::new(&self.msg_recorder)
    }

    pub(crate) fn ledger_stats(&self) -> LedgerStatsViewModel {
        LedgerStatsViewModel::new(&self.ledger_stats)
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
        assert_eq!(model.message_stats().messages_sent(), "0");
    }

    #[tokio::test]
    async fn starting_node() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());

        model.node_runner.start_beta_node();

        assert_eq!(model.node_runner.can_start_node(), false);
        assert_eq!(model.node_runner.can_stop_node(), false);
        assert_eq!(model.node_runner.status(), "starting...");
        assert_eq!(runtime.blocking_spawns(), 1);
    }

    #[tokio::test]
    async fn starting_completed() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());
        model.node_runner.start_beta_node();

        runtime.run_nulled_blocking_task();

        assert_eq!(model.node_runner.status(), "running");
        assert_eq!(model.node_runner.can_start_node(), false);
        assert_eq!(model.node_runner.can_stop_node(), true);
    }

    #[tokio::test]
    async fn stopping_node() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppViewModel::new(runtime.clone(), NodeFactory::new_null());
        model.node_runner.start_beta_node();
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
        model.node_runner.start_beta_node();
        runtime.run_nulled_blocking_task();
        model.node_runner.stop_node();
        runtime.run_nulled_blocking_task();
        assert_eq!(model.node_runner.can_start_node(), true);
        assert_eq!(model.node_runner.can_stop_node(), false);
        assert_eq!(model.node_runner.status(), "not running");
    }
}
