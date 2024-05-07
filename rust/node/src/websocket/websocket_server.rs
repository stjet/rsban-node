use super::{MessageBuilder, WebsocketListener};
use crate::{
    config::WebsocketConfig,
    consensus::{ActiveTransactions, ElectionStatus, ElectionStatusType, VoteProcessor},
    utils::AsyncRuntime,
    wallets::Wallets,
    websocket::Topic,
    Telemetry,
};
use rsnano_core::{Account, Amount, BlockType, VoteWithWeightInfo};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tracing::error;

pub fn create_websocket_server(
    config: WebsocketConfig,
    wallets: Arc<Wallets>,
    async_rt: Arc<AsyncRuntime>,
    active_transactions: &ActiveTransactions,
    telemetry: &Telemetry,
    vote_processor: &VoteProcessor,
) -> Option<Arc<WebsocketListener>> {
    if !config.enabled {
        return None;
    }

    let Ok(address) = config.address.parse::<IpAddr>() else {
        error!(address = config.address, "invalid websocket IP address");
        return None;
    };

    let endpoint = SocketAddr::new(address, config.port);
    let server = Arc::new(WebsocketListener::new(endpoint, wallets, async_rt));

    let server_w = Arc::downgrade(&server);
    active_transactions.add_election_end_callback(Box::new(
        move |status: &ElectionStatus,
              votes: &Vec<VoteWithWeightInfo>,
              account: Account,
              amount: Amount,
              is_state_send: bool,
              is_state_epoch: bool| {
            if let Some(server) = server_w.upgrade() {
                debug_assert!(status.election_status_type != ElectionStatusType::Ongoing);

                if server.any_subscriber(Topic::Confirmation) {
                    let block = status.winner.as_ref().unwrap();
                    let subtype = if is_state_send {
                        "send"
                    } else if block.block_type() == BlockType::State {
                        if block.is_change() {
                            "change"
                        } else if is_state_epoch {
                            "epoch"
                        } else {
                            "receive"
                        }
                    } else {
                        ""
                    };

                    server.broadcast_confirmation(block, &account, &amount, subtype, status, votes);
                }
            }
        },
    ));

    let server_w = Arc::downgrade(&server);
    active_transactions.add_active_started_callback(Box::new(move |hash| {
        if let Some(server) = server_w.upgrade() {
            if server.any_subscriber(Topic::StartedElection) {
                server.broadcast(&MessageBuilder::started_election(&hash).unwrap());
            }
        }
    }));

    let server_w = Arc::downgrade(&server);
    active_transactions.add_active_stopped_callback(Box::new(move |hash| {
        if let Some(server) = server_w.upgrade() {
            if server.any_subscriber(Topic::StoppedElection) {
                server.broadcast(&MessageBuilder::stopped_election(&hash).unwrap());
            }
        }
    }));

    let server_w = Arc::downgrade(&server);
    telemetry.add_callback(Box::new(move |data, channel| {
        if let Some(server) = server_w.upgrade() {
            if server.any_subscriber(Topic::Telemetry) {
                server.broadcast(
                    &MessageBuilder::telemetry_received(data, channel.remote_endpoint()).unwrap(),
                );
            }
        }
    }));

    let server_w = Arc::downgrade(&server);
    vote_processor.add_vote_processed_callback(Box::new(move |vote, _channel, vote_code| {
        if let Some(server) = server_w.upgrade() {
            if server.any_subscriber(Topic::Vote) {
                server.broadcast(&MessageBuilder::vote_received(vote, vote_code).unwrap());
            }
        }
    }));

    Some(server)
}
