use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::PropertyTreeWriter;

#[derive(Clone, Copy, FromPrimitive)]
pub(crate) enum Topic {
    Invalid = 0,
    /// Acknowledgement of prior incoming message
    Ack,
    /// A confirmation message
    Confirmation,
    /// Stopped election message (dropped elections due to bounding or block lost the elections)
    StoppedElection,
    /// A vote message
    Vote,
    /// Work generation message
    Work,
    /// A bootstrap message
    Bootstrap,
    /// A telemetry message
    Telemetry,
    /// New block arrival message
    NewUnconfirmedBlock,
    /// Auxiliary length, not a valid topic, must be the last enum
    Length,
}

pub(crate) struct Message {
    pub topic: Topic,
    pub contents: Box<dyn PropertyTreeWriter>,
}

pub(crate) struct MessageBuilder {}

impl MessageBuilder {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) fn set_common_fields(message: &mut Message) -> Result<()> {
        message.contents.add("topic", from_topic(message.topic))?;
        message.contents.add(
            "time",
            &SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string(),
        )?;
        Ok(())
    }
}

pub(crate) fn from_topic(topic: Topic) -> &'static str {
    match topic {
        Topic::Ack => "ack",
        Topic::Confirmation => "confirmation",
        Topic::StoppedElection => "stopped_election",
        Topic::Vote => "vote",
        Topic::Work => "work",
        Topic::Bootstrap => "bootstrap",
        Topic::Telemetry => "telemetry",
        Topic::NewUnconfirmedBlock => "new_unconfirmed_block",
        _ => "invalid",
    }
}
