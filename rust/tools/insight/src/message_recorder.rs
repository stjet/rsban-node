use rsnano_messages::Message;
use rsnano_network::{ChannelDirection, ChannelId};
use rsnano_node::NodeCallbacks;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
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
    messages: RwLock<Vec<RecordedMessage>>,
    is_recording: AtomicBool,
}

impl MessageRecorder {
    pub(crate) fn new() -> Self {
        Self {
            published: AtomicUsize::new(0),
            inbound: AtomicUsize::new(0),
            messages: RwLock::new(Vec::new()),
            is_recording: AtomicBool::new(false),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    pub fn start_recording(&self) {
        self.is_recording.store(true, Ordering::SeqCst);
    }

    pub fn stop_recording(&self) {
        self.is_recording.store(false, Ordering::SeqCst);
    }

    pub fn clear(&self) {
        self.messages.write().unwrap().clear();
    }

    pub fn record(&self, msg: RecordedMessage) {
        match msg.direction {
            ChannelDirection::Inbound => self.inbound.fetch_add(1, Ordering::SeqCst),
            ChannelDirection::Outbound => self.published.fetch_add(1, Ordering::SeqCst),
        };

        if !self.is_recording() {
            return;
        }
        self.messages.write().unwrap().push(msg);
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
