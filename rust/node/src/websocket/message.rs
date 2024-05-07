use super::ConfirmationOptions;
use crate::{
    consensus::{ElectionStatus, ElectionStatusType},
    DEV_NETWORK_PARAMS,
};
use anyhow::Result;
use rsnano_core::{
    utils::{PropertyTree, SerdePropertyTree},
    Account, Amount, BlockEnum, BlockHash, DifficultyV1, Vote, VoteCode, VoteWithWeightInfo,
    WorkVersion,
};
use rsnano_messages::TelemetryData;
use std::{
    fmt::Debug,
    net::SocketAddrV6,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, FromPrimitive, PartialEq, Eq, Hash)]
pub enum Topic {
    Invalid = 0,
    /// Acknowledgement of prior incoming message
    Ack,
    /// A confirmation message
    Confirmation,
    StartedElection,
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

impl Topic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Topic::Ack => "ack",
            Topic::Confirmation => "confirmation",
            Topic::StartedElection => "started_election",
            Topic::StoppedElection => "stopped_election",
            Topic::Vote => "vote",
            Topic::Work => "work",
            Topic::Bootstrap => "bootstrap",
            Topic::Telemetry => "telemetry",
            Topic::NewUnconfirmedBlock => "new_unconfirmed_block",
            _ => "invalid",
        }
    }
}

impl Debug for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct Message {
    pub topic: Topic,
    pub contents: SerdePropertyTree,
}

impl Clone for Message {
    fn clone(&self) -> Self {
        Self {
            topic: self.topic,
            contents: self.contents.clone(),
        }
    }
}

impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("topic", &self.topic)
            .finish()
    }
}

impl Message {
    pub fn new(topic: Topic) -> Self {
        Self {
            topic,
            contents: SerdePropertyTree::new(),
        }
    }
}

/// Message builder. This is expanded with new builder functions are necessary.
pub struct MessageBuilder {}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn bootstrap_started(id: &str, mode: &str) -> Result<Message> {
        let mut message = Self::new_message()?;
        // Bootstrap information
        let mut bootstrap = SerdePropertyTree::new();
        bootstrap.put_string("reason", "started")?;
        bootstrap.put_string("id", id)?;
        bootstrap.put_string("mode", mode)?;
        message.contents.add_child("message", &bootstrap);
        Ok(message)
    }

    pub fn bootstrap_exited(
        id: &str,
        mode: &str,
        duration: Duration,
        total_blocks: u64,
    ) -> Result<Message> {
        let mut message = Self::new_message()?;
        let mut bootstrap = SerdePropertyTree::new();
        bootstrap.put_string("reason", "exited")?;
        bootstrap.put_string("id", id)?;
        bootstrap.put_string("mode", mode)?;
        bootstrap.put_u64("total_blocks", total_blocks)?;
        bootstrap.put_u64("duration", duration.as_secs())?;
        message.contents.add_child("message", &bootstrap);

        Ok(message)
    }

    pub fn telemetry_received(
        telemetry_data: &TelemetryData,
        endpoint: SocketAddrV6,
    ) -> Result<Message> {
        let mut message_l = Message::new(Topic::Telemetry);
        Self::set_common_fields(&mut message_l)?;

        // Telemetry information
        let mut telemetry_l = SerdePropertyTree::new();
        telemetry_data.serialize_json(&mut telemetry_l, false)?;
        telemetry_l.put_string("address", &endpoint.ip().to_string())?;
        telemetry_l.put_u64("port", endpoint.port() as u64)?;

        message_l.contents.add_child("message", &telemetry_l);
        Ok(message_l)
    }

    pub fn new_block_arrived(block: &BlockEnum) -> Result<Message> {
        let mut message_l = Message::new(Topic::NewUnconfirmedBlock);
        Self::set_common_fields(&mut message_l)?;

        let mut block_l = SerdePropertyTree::new();
        block.serialize_json(&mut block_l)?;
        let subtype = block.sideband().unwrap().details.state_subtype();
        block_l.put_string("subtype", subtype)?;

        message_l.contents.add_child("message", &block_l);

        Ok(message_l)
    }

    pub fn started_election(hash: &BlockHash) -> Result<Message> {
        let mut message = Message::new(Topic::StartedElection);
        Self::set_common_fields(&mut message)?;

        let mut message_node_l = SerdePropertyTree::new();
        message_node_l.add("hash", &hash.to_string())?;
        message.contents.add_child("message", &message_node_l);
        Ok(message)
    }

    pub fn stopped_election(hash: &BlockHash) -> Result<Message> {
        let mut message = Message::new(Topic::StoppedElection);
        Self::set_common_fields(&mut message)?;

        let mut message_node_l = SerdePropertyTree::new();
        message_node_l.add("hash", &hash.to_string())?;
        message.contents.add_child("message", &message_node_l);
        Ok(message)
    }

    pub fn block_confirmed(
        block_a: &Arc<BlockEnum>,
        account_a: &Account,
        amount_a: &Amount,
        subtype: String,
        include_block_a: bool,
        election_status_a: &ElectionStatus,
        election_votes_a: &[VoteWithWeightInfo],
        options_a: &ConfirmationOptions,
    ) -> Result<Message> {
        let mut message_l = Message::new(Topic::Confirmation);
        Self::set_common_fields(&mut message_l)?;

        // Block confirmation properties
        let mut message_node_l = SerdePropertyTree::new();
        message_node_l.add("account", &account_a.encode_account())?;
        message_node_l.add("amount", &amount_a.to_string_dec())?;
        message_node_l.add("hash", &block_a.hash().to_string())?;

        let confirmation_type = match election_status_a.election_status_type {
            ElectionStatusType::ActiveConfirmedQuorum => "active_quorum",
            ElectionStatusType::ActiveConfirmationHeight => "active_confirmation_height",
            ElectionStatusType::InactiveConfirmationHeight => "inactive",
            _ => "unknown",
        };
        message_node_l.add("confirmation_type", confirmation_type)?;

        if options_a.include_election_info || options_a.include_election_info_with_votes {
            let mut election_node_l = SerdePropertyTree::new();
            election_node_l.add(
                "duration",
                &election_status_a.election_duration.as_millis().to_string(),
            )?;
            election_node_l.add(
                "time",
                &election_status_a
                    .election_end
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .to_string(),
            )?;
            election_node_l.add("tally", &election_status_a.tally.to_string_dec())?;
            election_node_l.add("final", &election_status_a.final_tally.to_string_dec())?;
            election_node_l.add("blocks", &election_status_a.block_count.to_string())?;
            election_node_l.add("voters", &election_status_a.voter_count.to_string())?;
            election_node_l.add(
                "request_count",
                &election_status_a.confirmation_request_count.to_string(),
            )?;
            if options_a.include_election_info_with_votes {
                let mut election_votes_l = Vec::new();
                for vote_l in election_votes_a {
                    let mut entry = SerdePropertyTree::new();
                    entry.put_string("representative", &vote_l.representative.encode_account())?;
                    entry.put_string("timestamp", &vote_l.timestamp.to_string())?;
                    entry.put_string("hash", &vote_l.hash.to_string())?;
                    entry.put_string("weight", &vote_l.weight.to_string_dec())?;
                    election_votes_l.push(entry.value);
                }
                election_node_l.add_child_value(
                    "votes".to_string(),
                    serde_json::Value::Array(election_votes_l),
                );
            }
            message_node_l.add_child("election_info", &election_node_l);
        }

        if include_block_a {
            let mut block_node_l = SerdePropertyTree::new();
            block_a.serialize_json(&mut block_node_l)?;
            if !subtype.is_empty() {
                block_node_l.add("subtype", &subtype)?;
            }
            message_node_l.add_child("block", &block_node_l);
        }

        if options_a.include_sideband_info {
            let mut sideband_node_l = SerdePropertyTree::new();
            sideband_node_l.add("height", &block_a.sideband().unwrap().height.to_string())?;
            sideband_node_l.add(
                "local_timestamp",
                &block_a.sideband().unwrap().timestamp.to_string(),
            )?;
            message_node_l.add_child("sideband", &sideband_node_l);
        }

        message_l.contents.add_child("message", &message_node_l);

        Ok(message_l)
    }

    pub fn vote_received(vote_a: &Arc<Vote>, code_a: VoteCode) -> Result<Message> {
        let mut message_l = Message::new(Topic::Vote);
        Self::set_common_fields(&mut message_l)?;

        // Vote information
        let mut vote_node_l = vote_a.serialize_json();

        // Vote processing information
        let vote_type = match code_a {
            VoteCode::Vote => "vote",
            VoteCode::Replay => "replay",
            VoteCode::Indeterminate => "indeterminate",
            VoteCode::Ignored => "ignored",
            VoteCode::Invalid => unreachable!(),
        };

        let serde_json::Value::Object(o) = &mut vote_node_l else {
            unreachable!()
        };
        o.insert(
            "type".to_string(),
            serde_json::Value::String(vote_type.to_string()),
        );
        message_l
            .contents
            .add_child("message", &SerdePropertyTree::from_value(vote_node_l));
        Ok(message_l)
    }

    pub fn work_generation(
        version_a: WorkVersion,
        root_a: &BlockHash,
        work_a: u64,
        difficulty_a: u64,
        publish_threshold_a: u64,
        duration_a: Duration,
        peer_a: &str,
        bad_peers_a: &[String],
        completed_a: bool,
        cancelled_a: bool,
    ) -> Result<Message> {
        let mut message_l = Message::new(Topic::Work);
        Self::set_common_fields(&mut message_l)?;

        // Active difficulty information
        let mut work_l = SerdePropertyTree::new();
        work_l.put_string("success", if completed_a { "true" } else { "false" })?;
        work_l.put_string(
            "reason",
            if completed_a {
                ""
            } else if cancelled_a {
                "cancelled"
            } else {
                "failure"
            },
        )?;
        work_l.put_u64("duration", duration_a.as_millis() as u64)?;

        let mut request_l = SerdePropertyTree::new();
        request_l.put_string("version", version_a.as_str())?;
        request_l.put_string("hash", &root_a.to_string())?;
        request_l.put_string("difficulty", &format!("{:016x}", difficulty_a))?;
        let request_multiplier_l = DifficultyV1::to_multiplier(difficulty_a, publish_threshold_a);
        request_l.put_string("multiplier", &format!("{:.10}", request_multiplier_l))?;
        work_l.add_child("request", &request_l);

        if completed_a {
            let mut result_l = SerdePropertyTree::new();
            result_l.put_string("source", peer_a)?;
            result_l.put_string("work", &format!("{:016x}", work_a))?;
            let result_difficulty_l =
                DEV_NETWORK_PARAMS
                    .work
                    .difficulty(version_a, &root_a.into(), work_a);
            result_l.put_string("difficulty", &format!("{:016x}", result_difficulty_l))?;
            let result_multiplier_l =
                DifficultyV1::to_multiplier(result_difficulty_l, publish_threshold_a);
            result_l.put_string("multiplier", &format!("{:.10}", result_multiplier_l))?;
            work_l.add_child("result", &result_l);
        }

        let mut bad_peers_l = Vec::new();
        for peer_text in bad_peers_a {
            let mut entry = SerdePropertyTree::new();
            entry.put_string("", peer_text)?;
            bad_peers_l.push(entry.value);
        }
        work_l.add_child_value(
            "bad_peers".to_string(),
            serde_json::Value::Array(bad_peers_l),
        );

        message_l.contents.add_child("message", &work_l);
        Ok(message_l)
    }

    pub fn set_common_fields(message: &mut Message) -> Result<()> {
        message.contents.add("topic", message.topic.as_str())?;
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

    fn new_message() -> Result<Message, anyhow::Error> {
        let mut message = Message {
            topic: Topic::Bootstrap,
            contents: SerdePropertyTree::new(),
        };
        Self::set_common_fields(&mut message)?;
        Ok(message)
    }
}

pub fn to_topic(topic: impl AsRef<str>) -> Topic {
    match topic.as_ref() {
        "confirmation" => Topic::Confirmation,
        "started_election" => Topic::StartedElection,
        "stopped_election" => Topic::StoppedElection,
        "vote" => Topic::Vote,
        "ack" => Topic::Ack,
        "work" => Topic::Work,
        "bootstrap" => Topic::Bootstrap,
        "telemetry" => Topic::Telemetry,
        "new_unconfirmed_block" => Topic::NewUnconfirmedBlock,
        _ => Topic::Invalid,
    }
}
