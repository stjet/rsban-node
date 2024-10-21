use super::MessageViewModel;
use crate::message_collection::MessageCollection;
use rsnano_messages::MessageType;
use rsnano_network::ChannelDirection;
use std::sync::{Arc, RwLock};

pub(crate) struct RowViewModel {
    pub channel_id: String,
    pub direction: String,
    pub message: String,
    pub is_selected: bool,
}

pub(crate) struct MessageTableViewModel {
    selected: Option<MessageViewModel>,
    selected_index: Option<usize>,
    messages: Arc<RwLock<MessageCollection>>,
    pub message_types: Vec<MessageTypeOptionViewModel>,
}

impl MessageTableViewModel {
    pub(crate) fn new(messages: Arc<RwLock<MessageCollection>>) -> Self {
        Self {
            messages,
            selected: None,
            selected_index: None,
            message_types: Vec::new(),
        }
    }

    pub(crate) fn heading(&self) -> String {
        format!("Messages ({})", self.messages.read().unwrap().len())
    }

    pub(crate) fn get_row(&self, index: usize) -> Option<RowViewModel> {
        let message = self.messages.read().unwrap().get(index)?;
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
        self.messages.read().unwrap().len()
    }

    pub(crate) fn selected_message(&self) -> Option<MessageViewModel> {
        self.selected.clone()
    }

    pub(crate) fn select_message(&mut self, index: usize) {
        let message = self.messages.read().unwrap().get(index).unwrap();
        self.selected = Some(message.into());
        self.selected_index = Some(index);
    }

    pub(crate) fn update_filter(&self) {
        self.messages.write().unwrap().filter_message_types(
            self.message_types
                .iter()
                .filter(|i| i.selected)
                .map(|i| i.value),
        );
    }

    pub(crate) fn update_message_counts(&mut self) {
        let messages = self.messages.read().unwrap();
        let counts = messages.message_counts();
        let empty = Vec::with_capacity(counts.len());

        let old = std::mem::replace(&mut self.message_types, empty);
        for (msg_type, count) in counts {
            self.message_types.push(MessageTypeOptionViewModel {
                value: *msg_type,
                label: format!("{}({})", msg_type.as_str(), count),
                selected: false,
            })
        }

        for mut type_model in old {
            if type_model.selected {
                let mut found = false;
                for mt in self.message_types.iter_mut() {
                    if mt.value == type_model.value {
                        mt.selected = true;
                        found = true;
                        break;
                    }
                }

                if !found {
                    type_model.label = format!("{}({})", type_model.value.as_str(), 0);
                    self.message_types.push(type_model);
                }
            }
        }

        self.message_types.sort_by_key(|x| x.value as u8)
    }
}

pub(crate) struct MessageTypeOptionViewModel {
    pub value: MessageType,
    pub label: String,
    pub selected: bool,
}
