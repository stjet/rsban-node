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
    pub rates: MessageRates,
    rate_calc: RwLock<MessageRateCalculator>,
    messages: Arc<RwLock<MessageCollection>>,
    is_recording: AtomicBool,
}

impl MessageRecorder {
    pub(crate) fn new() -> Self {
        Self {
            rates: Default::default(),
            rate_calc: RwLock::new(Default::default()),
            messages: Arc::new(RwLock::new(MessageCollection::default())),
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

    pub fn record(&self, message: RecordedMessage, now: Timestamp) {
        {
            let mut rates = self.rate_calc.write().unwrap();
            rates.caluclate(&message, now, &self.rates);
        }

        if self.is_recording() {
            let mut messages = self.messages.write().unwrap();
            messages.add(message);
        }
    }

    pub fn get_message(&self, index: usize) -> Option<RecordedMessage> {
        self.messages.read().unwrap().get(index)
    }

    pub(crate) fn message_count(&self) -> usize {
        self.messages.read().unwrap().len()
    }
}

#[derive(Default)]
pub(crate) struct MessageRates {
    pub send_rate: AtomicU64,
    pub receive_rate: AtomicU64,
}

#[derive(Default)]
pub(crate) struct MessageRateCalculator {
    sent: u64,
    received: u64,
    receive_rate: RateCalculator,
    send_rate: RateCalculator,
    last_rate_sample: Option<Timestamp>,
}

impl MessageRateCalculator {
    pub fn caluclate(&mut self, message: &RecordedMessage, now: Timestamp, result: &MessageRates) {
        let should_sample = if let Some(ts) = self.last_rate_sample {
            (now - ts) >= Duration::from_millis(500)
        } else {
            true
        };

        match message.direction {
            ChannelDirection::Inbound => {
                self.received += 1;
                if should_sample {
                    self.receive_rate.sample(self.received, now);
                    result
                        .receive_rate
                        .store(self.receive_rate.rate(), Ordering::Relaxed);
                    self.last_rate_sample = Some(now);
                }
            }
            ChannelDirection::Outbound => {
                self.sent += 1;
                if should_sample {
                    self.send_rate.sample(self.sent, now);
                    result
                        .send_rate
                        .store(self.send_rate.rate(), Ordering::Relaxed);
                    self.last_rate_sample = Some(now);
                }
            }
        };
    }
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
