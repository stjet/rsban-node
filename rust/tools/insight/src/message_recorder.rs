use chrono::Utc;
use rsnano_network::ChannelDirection;
use rsnano_node::NodeCallbacks;
use rsnano_nullable_clock::{SteadyClock, Timestamp};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use crate::{
    message_collection::{MessageCollection, RecordedMessage},
    rate_calculator::RateCalculator,
};

pub(crate) struct MessageRecorder {
    pub sent: AtomicU64,
    pub received: AtomicU64,
    pub send_rate: AtomicU64,
    pub receive_rate: AtomicU64,
    data: RwLock<Data>,
    is_recording: AtomicBool,
}

impl MessageRecorder {
    pub(crate) fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
            send_rate: AtomicU64::new(0),
            receive_rate: AtomicU64::new(0),
            data: RwLock::new(Default::default()),
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
        self.data.write().unwrap().messages.clear();
    }

    pub fn record(&self, msg: RecordedMessage, now: Timestamp) {
        let mut guard = self.data.write().unwrap();

        let should_sample = if let Some(ts) = guard.last_rate_sample {
            (now - ts) >= Duration::from_millis(500)
        } else {
            true
        };

        match msg.direction {
            ChannelDirection::Inbound => {
                let received = self.received.fetch_add(1, Ordering::Relaxed) + 1;
                if should_sample {
                    guard.receive_rate.sample(received, now);
                    self.receive_rate
                        .store(guard.receive_rate.rate(), Ordering::Relaxed);
                    guard.last_rate_sample = Some(now);
                }
            }
            ChannelDirection::Outbound => {
                let sent = self.sent.fetch_add(1, Ordering::Relaxed) + 1;
                if should_sample {
                    guard.send_rate.sample(sent, now);
                    self.send_rate
                        .store(guard.send_rate.rate(), Ordering::Relaxed);
                    guard.last_rate_sample = Some(now);
                }
            }
        };

        if self.is_recording() {
            guard.messages.add(msg);
        }
    }

    pub fn get_message(&self, index: usize) -> Option<RecordedMessage> {
        self.data.read().unwrap().messages.get(index)
    }

    pub(crate) fn message_count(&self) -> usize {
        self.data.read().unwrap().messages.len()
    }
}

#[derive(Default)]
struct Data {
    messages: MessageCollection,
    receive_rate: RateCalculator,
    send_rate: RateCalculator,
    last_rate_sample: Option<Timestamp>,
}

pub(crate) fn make_node_callbacks(
    recorder: Arc<MessageRecorder>,
    clock: Arc<SteadyClock>,
) -> NodeCallbacks {
    let recorder2 = recorder.clone();
    let recorder3 = recorder.clone();
    let clock2 = clock.clone();
    let clock3 = clock.clone();
    NodeCallbacks::builder()
        .on_publish(move |channel_id, message| {
            let recorded = RecordedMessage {
                channel_id,
                message: message.clone(),
                direction: ChannelDirection::Outbound,
                date: Utc::now(),
            };
            recorder.record(recorded, clock.now());
        })
        .on_inbound(move |channel_id, message| {
            let recorded = RecordedMessage {
                channel_id,
                message: message.clone(),
                direction: ChannelDirection::Inbound,
                date: Utc::now(),
            };
            recorder2.record(recorded, clock2.now());
        })
        .on_inbound_dropped(move |channel_id, message| {
            let recorded = RecordedMessage {
                channel_id,
                message: message.clone(),
                direction: ChannelDirection::Inbound,
                date: Utc::now(),
            };
            recorder3.record(recorded, clock3.now());
        })
        .finish()
}
