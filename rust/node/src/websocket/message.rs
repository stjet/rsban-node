use super::ConfirmationOptions;
use crate::{
    consensus::{ElectionStatus, ElectionStatusType},
    DEV_NETWORK_PARAMS,
};
use anyhow::Result;
use rsnano_core::{
    utils::{milliseconds_since_epoch, PropertyTree, SerdePropertyTree},
    Account, Amount, BlockEnum, BlockHash, DifficultyV1, Vote, VoteCode, VoteWithWeightInfo,
    WorkVersion,
};
use rsnano_messages::TelemetryData;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt::Debug,
    hash::Hash,
    net::SocketAddrV6,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, FromPrimitive, PartialEq, Eq, Hash, Serialize, Debug)]
#[serde(rename_all = "snake_case")]
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

#[derive(Deserialize)]
pub struct IncomingMessage<'a> {
    pub action: Option<&'a str>,
    pub topic: Option<&'a str>,
    #[serde(default)]
    pub ack: bool,
    pub id: Option<&'a str>,
    pub options: Option<Value>,
    #[serde(default)]
    pub accounts_add: Vec<&'a str>,
    #[serde(default)]
    pub accounts_del: Vec<&'a str>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OutgoingMessageEnvelope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<Topic>,
    pub time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Value>,
}

impl OutgoingMessageEnvelope {
    pub fn new(topic: Topic, message: impl Serialize) -> Self {
        Self {
            id: None,
            ack: None,
            topic: Some(topic),
            time: milliseconds_since_epoch().to_string(),
            message: Some(serde_json::to_value(message).expect("could not serialize message")),
        }
    }

    pub fn new_ack(id: Option<String>, action: String) -> Self {
        Self {
            id,
            topic: None,
            ack: Some(action),
            time: milliseconds_since_epoch().to_string(),
            message: None,
        }
    }
}

#[derive(Serialize)]
struct BootstrapStarted<'a> {
    reason: &'a str,
    id: &'a str,
    mode: &'a str,
}

#[derive(Serialize)]
struct BootstrapExited<'a> {
    reason: &'a str,
    id: &'a str,
    mode: &'a str,
    total_blocks: String,
    duration: String,
}

#[derive(Serialize)]
struct TelemetryReceived {
    block_count: String,
    cemented_count: String,
    unchecked_count: String,
    account_count: String,
    bandwidth_cap: String,
    peer_count: String,
    protocol_version: String,
    uptime: String,
    genesis_block: String,
    major_version: String,
    minor_version: String,
    patch_version: String,
    pre_release_version: String,
    maker: String,
    timestamp: String,
    active_difficulty: String,
    node_id: String,
    signature: String,
    address: String,
    port: String,
}

#[derive(Serialize)]
struct StartedElection {
    hash: String,
}

#[derive(Serialize)]
struct StoppedElection {
    hash: String,
}

#[derive(Serialize)]
struct BlockConfirmed {
    account: String,
    amount: String,
    hash: String,
    confirmation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    election_info: Option<ElectionInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sideband: Option<JsonSideband>,
}

#[derive(Serialize)]
struct ElectionInfo {
    duration: String,
    time: String,
    tally: String,
    #[serde(rename = "final")]
    final_tally: String,
    blocks: String,
    voters: String,
    request_count: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    votes: Option<Vec<JsonVoteSummary>>,
}

#[derive(Serialize)]
struct JsonVoteSummary {
    representative: String,
    timestamp: String,
    hash: String,
    weight: String,
}

#[derive(Serialize)]
struct JsonSideband {
    height: String,
    local_timestamp: String,
}

#[derive(Serialize)]
struct VoteReceived {
    account: String,
    signature: String,
    sequence: String,
    timestamp: String,
    duration: String,
    blocks: Vec<String>,
    #[serde(rename = "type")]
    vote_type: String,
}

#[derive(Serialize)]
struct WorkGeneration<'a> {
    success: &'a str,
    reason: &'a str,
    duration: String,
    request: WorkRequest<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<WorkResult>,
    bad_peers: Vec<String>,
}

#[derive(Serialize)]
struct WorkRequest<'a> {
    version: &'a str,
    hash: String,
    difficulty: String,
    multiplier: String,
}

#[derive(Serialize)]
struct WorkResult {
    source: String,
    work: String,
    difficulty: String,
    multiplier: String,
}

/// Message builder. This is expanded with new builder functions are necessary.
pub struct MessageBuilder {}

impl MessageBuilder {
    pub fn bootstrap_started(id: &str, mode: &str) -> Result<OutgoingMessageEnvelope> {
        {
            let topic = Topic::Bootstrap;
            let message = BootstrapStarted {
                reason: "started",
                id,
                mode,
            };
            Ok(OutgoingMessageEnvelope::new(topic, message))
        }
    }

    pub fn bootstrap_exited(
        id: &str,
        mode: &str,
        duration: Duration,
        total_blocks: u64,
    ) -> Result<OutgoingMessageEnvelope> {
        {
            let topic = Topic::Bootstrap;
            let message = BootstrapExited {
                reason: "exited",
                id,
                mode,
                total_blocks: total_blocks.to_string(),
                duration: duration.as_secs().to_string(),
            };
            Ok(OutgoingMessageEnvelope::new(topic, message))
        }
    }

    pub fn telemetry_received(
        data: &TelemetryData,
        endpoint: SocketAddrV6,
    ) -> Result<OutgoingMessageEnvelope> {
        {
            let topic = Topic::Telemetry;
            let message = TelemetryReceived {
                block_count: data.block_count.to_string(),
                cemented_count: data.cemented_count.to_string(),
                unchecked_count: data.unchecked_count.to_string(),
                account_count: data.account_count.to_string(),
                bandwidth_cap: data.bandwidth_cap.to_string(),
                peer_count: data.peer_count.to_string(),
                protocol_version: data.protocol_version.to_string(),
                uptime: data.uptime.to_string(),
                genesis_block: data.genesis_block.to_string(),
                major_version: data.major_version.to_string(),
                minor_version: data.minor_version.to_string(),
                patch_version: data.patch_version.to_string(),
                pre_release_version: data.pre_release_version.to_string(),
                maker: data.maker.to_string(),
                timestamp: data
                    .timestamp
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .to_string(),
                active_difficulty: format!("{:016x}", data.active_difficulty),
                node_id: data.node_id.to_node_id(),
                signature: data.signature.encode_hex(),
                address: endpoint.ip().to_string(),
                port: endpoint.port().to_string(),
            };
            Ok(OutgoingMessageEnvelope::new(topic, message))
        }
    }

    pub fn new_block_arrived(block: &BlockEnum) -> OutgoingMessageEnvelope {
        let mut json_block = SerdePropertyTree::new();
        block.serialize_json(&mut json_block).unwrap();
        let subtype = block.sideband().unwrap().details.state_subtype();
        json_block.put_string("subtype", subtype).unwrap();
        OutgoingMessageEnvelope::new(Topic::NewUnconfirmedBlock, json_block.value)
    }

    pub fn started_election(hash: &BlockHash) -> Result<OutgoingMessageEnvelope> {
        {
            let topic = Topic::StartedElection;
            let message = StartedElection {
                hash: hash.to_string(),
            };
            Ok(OutgoingMessageEnvelope::new(topic, message))
        }
    }

    pub fn stopped_election(hash: &BlockHash) -> Result<OutgoingMessageEnvelope> {
        {
            let topic = Topic::StoppedElection;
            let message = StoppedElection {
                hash: hash.to_string(),
            };
            Ok(OutgoingMessageEnvelope::new(topic, message))
        }
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
    ) -> Result<OutgoingMessageEnvelope> {
        let confirmation_type = match election_status_a.election_status_type {
            ElectionStatusType::ActiveConfirmedQuorum => "active_quorum",
            ElectionStatusType::ActiveConfirmationHeight => "active_confirmation_height",
            ElectionStatusType::InactiveConfirmationHeight => "inactive",
            _ => "unknown",
        };

        let election_info =
            if options_a.include_election_info || options_a.include_election_info_with_votes {
                let votes = if options_a.include_election_info_with_votes {
                    Some(
                        election_votes_a
                            .iter()
                            .map(|v| JsonVoteSummary {
                                representative: v.representative.encode_account(),
                                timestamp: v.timestamp.to_string(),
                                hash: v.hash.to_string(),
                                weight: v.weight.to_string_dec(),
                            })
                            .collect(),
                    )
                } else {
                    None
                };

                Some(ElectionInfo {
                    duration: election_status_a.election_duration.as_millis().to_string(),
                    time: election_status_a
                        .election_end
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        .to_string(),
                    tally: election_status_a.tally.to_string_dec(),
                    final_tally: election_status_a.final_tally.to_string_dec(),
                    blocks: election_status_a.block_count.to_string(),
                    voters: election_status_a.voter_count.to_string(),
                    request_count: election_status_a.confirmation_request_count.to_string(),
                    votes,
                })
            } else {
                None
            };

        let block = if include_block_a {
            let mut block_node_l = SerdePropertyTree::new();
            block_a.serialize_json(&mut block_node_l)?;
            if !subtype.is_empty() {
                block_node_l.add("subtype", &subtype)?;
            }
            Some(block_node_l.value)
        } else {
            None
        };

        let sideband = if options_a.include_sideband_info {
            Some(JsonSideband {
                height: block_a.sideband().unwrap().height.to_string(),
                local_timestamp: block_a.sideband().unwrap().timestamp.to_string(),
            })
        } else {
            None
        };

        let confirmed = BlockConfirmed {
            account: account_a.encode_account(),
            amount: amount_a.to_string_dec(),
            hash: block_a.hash().to_string(),
            confirmation_type: confirmation_type.to_string(),
            election_info,
            block,
            sideband,
        };

        {
            let topic = Topic::Confirmation;
            Ok(OutgoingMessageEnvelope::new(topic, confirmed))
        }
    }

    pub fn vote_received(vote_a: &Arc<Vote>, code_a: VoteCode) -> Result<OutgoingMessageEnvelope> {
        let vote_type = match code_a {
            VoteCode::Vote => "vote",
            VoteCode::Replay => "replay",
            VoteCode::Indeterminate => "indeterminate",
            VoteCode::Ignored => "ignored",
            VoteCode::Invalid => unreachable!(),
        };

        {
            let topic = Topic::Vote;
            let message = VoteReceived {
                account: vote_a.voting_account.encode_account(),
                signature: vote_a.signature.encode_hex(),
                sequence: vote_a.timestamp().to_string(),
                timestamp: vote_a.timestamp().to_string(),
                duration: vote_a.duration_bits().to_string(),
                blocks: vote_a.hashes.iter().map(|h| h.to_string()).collect(),
                vote_type: vote_type.to_string(),
            };
            Ok(OutgoingMessageEnvelope::new(topic, message))
        }
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
    ) -> Result<OutgoingMessageEnvelope> {
        let request_multiplier_l = DifficultyV1::to_multiplier(difficulty_a, publish_threshold_a);
        let request = WorkRequest {
            version: version_a.as_str(),
            hash: root_a.to_string(),
            difficulty: format!("{:016x}", difficulty_a),
            multiplier: format!("{:.10}", request_multiplier_l),
        };

        let result = if completed_a {
            let result_difficulty_l =
                DEV_NETWORK_PARAMS
                    .work
                    .difficulty(version_a, &root_a.into(), work_a);

            let result_multiplier_l =
                DifficultyV1::to_multiplier(result_difficulty_l, publish_threshold_a);

            Some(WorkResult {
                source: peer_a.to_string(),
                work: format!("{:016x}", work_a),
                difficulty: format!("{:016x}", result_difficulty_l),
                multiplier: format!("{:.10}", result_multiplier_l),
            })
        } else {
            None
        };

        let bad_peers = bad_peers_a.iter().cloned().collect();
        let work_l = WorkGeneration {
            success: if completed_a { "true" } else { "false" },
            reason: if completed_a {
                ""
            } else if cancelled_a {
                "cancelled"
            } else {
                "failure"
            },
            duration: duration_a.as_millis().to_string(),
            request,
            result,
            bad_peers,
        };

        {
            let topic = Topic::Work;
            Ok(OutgoingMessageEnvelope::new(topic, work_l))
        }
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
