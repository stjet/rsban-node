use num_derive::FromPrimitive;
use rsnano_messages::Message;
use rsnano_network::{ChannelDirection, ChannelId};
use rsnano_node::NodeCallbacks;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, RwLock,
};

#[derive(Clone)]
pub(crate) struct RecordedMessage {
    pub channel_id: ChannelId,
    pub message: Message,
    pub direction: ChannelDirection,
}

pub(crate) struct MessageRecorder {
    pub published: AtomicUsize,
    pub inbound: AtomicUsize,
    pub messages: RwLock<Vec<RecordedMessage>>,
}

impl MessageRecorder {
    pub(crate) fn new() -> Self {
        Self {
            published: AtomicUsize::new(0),
            inbound: AtomicUsize::new(0),
            messages: RwLock::new(Vec::new()),
        }
    }

    pub fn record(&self, msg: RecordedMessage) {
        let direction = msg.direction;
        self.messages.write().unwrap().push(msg);
        match direction {
            ChannelDirection::Inbound => self.inbound.fetch_add(1, Ordering::SeqCst),
            ChannelDirection::Outbound => self.published.fetch_add(1, Ordering::SeqCst),
        };
    }

    pub fn get_message(&self, index: usize) -> Option<RecordedMessage> {
        self.messages.read().unwrap().get(index).cloned()
    }

    pub(crate) fn message_count(&self) -> usize {
        self.messages.read().unwrap().len()
    }
}

pub(crate) fn make_node_callbacks(recorder: Arc<MessageRecorder>) -> NodeCallbacks {
    let recorder2 = recorder.clone();
    let recorder3 = recorder.clone();
    NodeCallbacks::builder()
        .on_publish(move |channel_id, message| {
            let recorded = RecordedMessage {
                channel_id,
                message: message.clone(),
                direction: ChannelDirection::Outbound,
            };
            recorder.record(recorded);
        })
        .on_inbound(move |channel_id, message| {
            let recorded = RecordedMessage {
                channel_id,
                message: message.clone(),
                direction: ChannelDirection::Inbound,
            };
            recorder2.record(recorded);
        })
        .on_inbound_dropped(move |channel_id, message| {
            let recorded = RecordedMessage {
                channel_id,
                message: message.clone(),
                direction: ChannelDirection::Inbound,
            };
            recorder3.record(recorded);
        })
        .finish()
}
