use super::SynCookies;
use crate::stats::{DetailType, Direction, StatType, Stats};
use rsnano_core::{BlockHash, KeyPair};
use rsnano_messages::{NodeIdHandshakeQuery, NodeIdHandshakeResponse};
use std::{
    net::SocketAddrV6,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Responsible for performing a correct handshake when connecting to another node
pub(crate) struct HandshakeProcess {
    genesis_hash: BlockHash,
    node_id: KeyPair,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    pub handshake_received: AtomicBool,
    remote_endpoint: SocketAddrV6,
}

impl HandshakeProcess {
    pub(crate) fn new(
        genesis_hash: BlockHash,
        node_id: KeyPair,
        syn_cookies: Arc<SynCookies>,
        stats: Arc<Stats>,
        remote_endpoint: SocketAddrV6,
    ) -> Self {
        Self {
            genesis_hash,
            node_id,
            syn_cookies,
            stats,
            handshake_received: AtomicBool::new(false),
            remote_endpoint,
        }
    }

    pub(crate) fn was_handshake_received(&self) -> bool {
        self.handshake_received.load(Ordering::SeqCst)
    }

    pub(crate) fn verify_response(
        &self,
        response: &NodeIdHandshakeResponse,
        remote_endpoint: &SocketAddrV6,
    ) -> bool {
        // Prevent connection with ourselves
        if response.node_id == self.node_id.public_key() {
            self.stats.inc_dir(
                StatType::Handshake,
                DetailType::InvalidNodeId,
                Direction::In,
            );
            return false; // Fail
        }

        // Prevent mismatched genesis
        if let Some(v2) = &response.v2 {
            if v2.genesis != self.genesis_hash {
                self.stats.inc_dir(
                    StatType::Handshake,
                    DetailType::InvalidGenesis,
                    Direction::In,
                );
                return false; // Fail
            }
        }

        let Some(cookie) = self.syn_cookies.cookie(remote_endpoint) else {
            self.stats.inc_dir(
                StatType::Handshake,
                DetailType::MissingCookie,
                Direction::In,
            );
            return false; // Fail
        };

        if response.validate(&cookie).is_err() {
            self.stats.inc_dir(
                StatType::Handshake,
                DetailType::InvalidSignature,
                Direction::In,
            );
            return false; // Fail
        }

        self.stats
            .inc_dir(StatType::Handshake, DetailType::Ok, Direction::In);
        true // OK
    }

    pub(crate) fn prepare_response(
        &self,
        query: &NodeIdHandshakeQuery,
        v2: bool,
    ) -> NodeIdHandshakeResponse {
        if v2 {
            NodeIdHandshakeResponse::new_v2(&query.cookie, &self.node_id, self.genesis_hash)
        } else {
            NodeIdHandshakeResponse::new_v1(&query.cookie, &self.node_id)
        }
    }

    pub(crate) fn prepare_query(
        &self,
        remote_endpoint: &SocketAddrV6,
    ) -> Option<NodeIdHandshakeQuery> {
        self.syn_cookies
            .assign(remote_endpoint)
            .map(|cookie| NodeIdHandshakeQuery { cookie })
    }
}
