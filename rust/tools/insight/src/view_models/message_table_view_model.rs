use super::MessageViewModel;
use crate::message_recorder::MessageRecorder;
use rsnano_network::ChannelDirection;
use std::sync::Arc;

pub(crate) struct RowViewModel {
    pub channel_id: String,
    pub direction: String,
    pub message: String,
    pub is_selected: bool,
}

pub(crate) struct MessageTableViewModel {
    selected: Option<MessageViewModel>,
    selected_index: Option<usize>,
    msg_recorder: Arc<MessageRecorder>,
}

impl MessageTableViewModel {
    pub(crate) fn new(msg_recorder: Arc<MessageRecorder>) -> Self {
        Self {
            msg_recorder,
            selected: None,
            selected_index: None,
        }
    }

    pub(crate) fn heading(&self) -> String {
        format!("Messages ({})", self.msg_recorder.message_count())
    }

    pub(crate) fn get_row(&self, index: usize) -> Option<RowViewModel> {
        let message = self.msg_recorder.get_message(index)?;
        Some(RowViewModel {
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

    pub(crate) fn selected_message(&self) -> Option<MessageViewModel> {
        self.selected.clone()
    }

    pub(crate) fn select_message(&mut self, index: usize) {
        let message = self.msg_recorder.get_message(index).unwrap();
        self.selected = Some(message.into());
        self.selected_index = Some(index);
    }
}
