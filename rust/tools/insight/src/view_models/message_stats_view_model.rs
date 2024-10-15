use crate::message_recorder::MessageRecorder;
use num_format::{Locale, ToFormattedString};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct MessageStatsViewModel<'a>(&'a MessageRecorder);

impl<'a> MessageStatsViewModel<'a> {
    pub fn new(recorder: &'a MessageRecorder) -> Self {
        Self(recorder)
    }

    pub(crate) fn send_rate(&self) -> String {
        Self::to_string(&self.0.send_rate)
    }

    pub(crate) fn receive_rate(&self) -> String {
        Self::to_string(&self.0.receive_rate)
    }

    pub(crate) fn captured(&self) -> String {
        self.0.message_count().to_formatted_string(&Locale::en)
    }

    fn to_string(value: &AtomicU64) -> String {
        value
            .load(Ordering::SeqCst)
            .to_formatted_string(&Locale::en)
    }
}
