use chrono::{DateTime, TimeZone, Utc};
use rsnano_messages::Message;
use rsnano_network::{ChannelDirection, ChannelId};

#[derive(Clone)]
pub(crate) struct RecordedMessage {
    pub channel_id: ChannelId,
    pub message: Message,
    pub direction: ChannelDirection,
    pub date: DateTime<Utc>,
}

impl RecordedMessage {
    #[allow(dead_code)]
    pub fn new_test_instance() -> Self {
        Self {
            channel_id: 42.into(),
            message: Message::BulkPush,
            direction: ChannelDirection::Outbound,
            date: Utc.with_ymd_and_hms(2024, 10, 18, 12, 59, 0).unwrap(),
        }
    }
}

#[derive(Default, PartialEq, Eq, Debug)]
pub(crate) struct MessageFilter {
    channel_id: Option<ChannelId>,
}

impl MessageFilter {
    pub fn all() -> Self {
        Default::default()
    }

    pub fn channel(channel_id: ChannelId) -> Self {
        Self {
            channel_id: Some(channel_id),
        }
    }

    pub fn include(&self, message: &RecordedMessage) -> bool {
        if let Some(channel_id) = self.channel_id {
            if message.channel_id != channel_id {
                return false;
            }
        }

        true
    }
}

#[derive(Default)]
pub(crate) struct MessageCollection {
    all_messages: Vec<RecordedMessage>,
    filtered: Vec<RecordedMessage>,
    filter: MessageFilter,
}

impl MessageCollection {
    pub fn get(&self, index: usize) -> Option<RecordedMessage> {
        self.filtered.get(index).cloned()
    }

    pub fn len(&self) -> usize {
        self.filtered.len()
    }

    pub fn add(&mut self, message: RecordedMessage) {
        if self.filter.include(&message) {
            self.filtered.push(message.clone());
        }
        self.all_messages.push(message);
    }

    pub fn clear(&mut self) {
        self.all_messages.clear();
        self.filtered.clear();
    }

    pub fn current_filter(&self) -> &MessageFilter {
        &self.filter
    }

    pub fn set_filter(&mut self, filter: MessageFilter) {
        self.filter = filter;
        self.filtered = self
            .all_messages
            .iter()
            .filter(|m| self.filter.include(m))
            .cloned()
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let collection = MessageCollection::default();
        assert_eq!(collection.len(), 0);
        assert!(collection.get(0).is_none());
    }

    #[test]
    fn add_message() {
        let mut collection = MessageCollection::default();
        collection.add(RecordedMessage::new_test_instance());
        assert_eq!(collection.len(), 1);
        assert!(collection.get(0).is_some());
    }

    #[test]
    fn clear() {
        let mut collection = MessageCollection::default();
        collection.add(RecordedMessage::new_test_instance());

        collection.clear();

        assert_eq!(collection.len(), 0);
        assert_eq!(collection.all_messages.len(), 0);
        assert_eq!(collection.filtered.len(), 0);
    }

    #[test]
    fn filter() {
        let mut collection = MessageCollection::default();

        let channel_id = ChannelId::from(2);
        let another_channel = ChannelId::from(3);

        let message1 = RecordedMessage {
            channel_id: another_channel,
            ..RecordedMessage::new_test_instance()
        };
        let message2 = RecordedMessage {
            channel_id,
            ..RecordedMessage::new_test_instance()
        };
        let message3 = RecordedMessage {
            channel_id: another_channel,
            ..RecordedMessage::new_test_instance()
        };
        let message4 = RecordedMessage {
            channel_id,
            ..RecordedMessage::new_test_instance()
        };
        collection.add(message1);
        collection.add(message2);
        collection.add(message3);
        collection.add(message4);

        collection.set_filter(MessageFilter::channel(channel_id));

        assert_eq!(collection.len(), 2);
        assert!(collection.get(0).is_some());
        assert!(collection.get(1).is_some());
        assert!(collection.get(2).is_none());
    }
}
