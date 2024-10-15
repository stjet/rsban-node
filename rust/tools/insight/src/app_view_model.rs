use crate::{
    message_recorder::{MessageRecorder, RecordedMessage},
    node_factory::NodeFactory,
    node_runner::NodeRunner,
    node_runner_view_model::NodeRunnerViewModel,
    nullable_runtime::NullableRuntime,
};
use num_format::{Locale, ToFormattedString};
use rsnano_network::ChannelDirection;
use std::sync::{atomic::Ordering, Arc};

pub(crate) struct AppViewModel {
    pub msg_recorder: Arc<MessageRecorder>,
    pub node_runner: NodeRunnerViewModel,
    selected: Option<MessageDetailsModel>,
    selected_index: Option<usize>,
    total_blocks: u64,
    cemented_blocks: u64,
}

impl AppViewModel {
    pub(crate) fn new(runtime: Arc<NullableRuntime>, node_factory: NodeFactory) -> Self {
        let node_runner = NodeRunner::new(runtime, node_factory);
        let msg_recorder = Arc::new(MessageRecorder::new());
        Self {
            node_runner: NodeRunnerViewModel::new(node_runner, msg_recorder.clone()),
            msg_recorder,
            selected: None,
            selected_index: None,
            total_blocks: 0,
            cemented_blocks: 0,
        }
    }

    pub(crate) fn with_runtime(runtime: tokio::runtime::Handle) -> Self {
        Self::new(
            Arc::new(NullableRuntime::new(runtime.clone())),
            NodeFactory::new(runtime),
        )
    }

    pub(crate) fn messages_sent(&self) -> String {
        self.msg_recorder
            .published
            .load(Ordering::SeqCst)
            .to_formatted_string(&Locale::en)
    }

    pub(crate) fn messages_received(&self) -> String {
        self.msg_recorder
            .inbound
            .load(Ordering::SeqCst)
            .to_formatted_string(&Locale::en)
    }

    pub(crate) fn get_row(&self, index: usize) -> Option<RowModel> {
        let message = self.msg_recorder.get_message(index)?;
        Some(RowModel {
            channel_id: message.channel_id.to_string(),
            direction: if message.direction == ChannelDirection::Inbound {
                "in".into()
            } else {
                "out".into()
            },
            message: format!("{:?}", message.message.message_type()),
            is_selected: self.selected_index == Some(index),
        })
    }

    pub(crate) fn message_count(&self) -> usize {
        self.msg_recorder.message_count()
    }

    pub(crate) fn selected_message(&self) -> Option<MessageDetailsModel> {
        self.selected.clone()
    }

    pub(crate) fn select_message(&mut self, index: usize) {
        let message = self.msg_recorder.get_message(index).unwrap();
        self.selected = Some(message.into());
        self.selected_index = Some(index);
    }

    pub(crate) fn update(&mut self) {
        if let Some(node) = self.node_runner.node() {
            self.total_blocks = node.ledger.block_count();
            self.cemented_blocks = node.ledger.cemented_count();
        }
    }

    pub(crate) fn block_count(&self) -> String {
        self.total_blocks.to_formatted_string(&Locale::en)
    }

    pub(crate) fn cemented_count(&self) -> String {
        self.cemented_blocks.to_formatted_string(&Locale::en)
    }
}

#[derive(Clone)]
pub(crate) struct MessageDetailsModel {
    pub channel_id: String,
    pub direction: String,
    pub message_type: String,
    pub message: String,
}

impl From<RecordedMessage> for MessageDetailsModel {
    fn from(value: RecordedMessage) -> Self {
        Self {
            channel_id: value.channel_id.to_string(),
            direction: if value.direction == ChannelDirection::Inbound {
                "in".into()
            } else {
                "out".into()
            },
            message_type: format!("{:?}", value.message.message_type()),
            message: format!("{:#?}", value.message),
        }
    }
}

pub(crate) struct RowModel {
    pub channel_id: String,
    pub direction: String,
    pub message: String,
    pub is_selected: bool,
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
        assert_eq!(model.messages_sent(), "0");
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
