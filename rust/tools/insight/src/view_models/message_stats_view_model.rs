use crate::message_recorder::MessageRecorder;
use num_format::{Locale, ToFormattedString};
use std::sync::atomic::Ordering;

pub(crate) struct MessageStatsViewModel<'a>(&'a MessageRecorder);

impl<'a> MessageStatsViewModel<'a> {
    pub fn new(recorder: &'a MessageRecorder) -> Self {
        Self(recorder)
    }

    pub(crate) fn messages_sent(&self) -> String {
        self.0
            .published
            .load(Ordering::SeqCst)
            .to_formatted_string(&Locale::en)
    }

    pub(crate) fn messages_received(&self) -> String {
        self.0
            .inbound
            .load(Ordering::SeqCst)
            .to_formatted_string(&Locale::en)
    }
}
