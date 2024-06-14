use super::LocalVoteHistory;
use crate::{
    consensus::VoteRouter,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, TrafficType},
};
use rsnano_core::{BlockEnum, BlockHash, Root, Vote};
use rsnano_ledger::Ledger;
use rsnano_messages::{Message, Publish};
use rsnano_store_lmdb::LmdbReadTransaction;
use std::{collections::HashSet, sync::Arc};

pub(super) struct RequestAggregatorImpl<'a> {
    local_votes: &'a LocalVoteHistory,
    ledger: &'a Ledger,
    vote_router: &'a VoteRouter,
    stats: &'a Stats,
    tx: &'a LmdbReadTransaction,
    channel: &'a ChannelEnum,

    pub to_generate: Vec<Arc<BlockEnum>>,
    pub to_generate_final: Vec<Arc<BlockEnum>>,
    pub cached_votes: Vec<Arc<Vote>>,
    pub cached_hashes: HashSet<BlockHash>,
}

impl<'a> RequestAggregatorImpl<'a> {
    pub fn new(
        local_votes: &'a LocalVoteHistory,
        ledger: &'a Ledger,
        vote_router: &'a VoteRouter,
        stats: &'a Stats,
        tx: &'a LmdbReadTransaction,
        channel: &'a ChannelEnum,
    ) -> Self {
        Self {
            local_votes,
            ledger,
            vote_router,
            stats,
            tx,
            channel,
            to_generate: Vec::new(),
            to_generate_final: Vec::new(),
            cached_votes: Vec::new(),
            cached_hashes: HashSet::new(),
        }
    }

    pub fn add_votes(&mut self, requests: &[(BlockHash, Root)]) {
        for (hash, root) in requests {
            if self.cached_hashes.contains(hash) {
                // Hashes already sent
                continue;
            }

            if !self.add_cached_votes(root, hash) {
                self.do_the_other_complex_thing(root, hash)
            }
        }

        self.deduplicate_votes();
    }

    fn add_cached_votes(&mut self, root: &Root, hash: &BlockHash) -> bool {
        let found_votes = self.local_votes.votes(root, hash, false);
        if found_votes.is_empty() {
            return false;
        }
        for vote in found_votes {
            for found_hash in &vote.hashes {
                self.cached_hashes.insert(*found_hash);
            }
            self.cached_votes.push(vote);
        }
        true
    }

    fn do_the_other_complex_thing(&mut self, root: &Root, hash: &BlockHash) {
        let mut generate_vote = true;
        let mut generate_final_vote = false;
        let mut block = None;

        // 2. Final votes
        let final_vote_hashes = self.ledger.store.final_vote.get(self.tx, *root);
        if !final_vote_hashes.is_empty() {
            generate_final_vote = true;
            block = self.ledger.any().get_block(self.tx, &final_vote_hashes[0]);
            // Allow same root vote
            if let Some(b) = &block {
                if final_vote_hashes.len() > 1 {
                    self.to_generate_final.push(Arc::new(b.clone()));
                    block = self.ledger.any().get_block(self.tx, &final_vote_hashes[1]);
                    debug_assert!(final_vote_hashes.len() == 2);
                }
            }
        }

        // 3. Election winner by hash
        if block.is_none() {
            if let Some(election) = self.vote_router.election(hash) {
                block = election
                    .mutex
                    .lock()
                    .unwrap()
                    .status
                    .winner
                    .as_ref()
                    .map(|b| (**b).clone())
            }
        }

        // 4. Ledger by hash
        if block.is_none() {
            block = self.ledger.any().get_block(self.tx, hash);
            // Confirmation status. Generate final votes for confirmed
            if let Some(b) = &block {
                let confirmation_height_info = self
                    .ledger
                    .store
                    .confirmation_height
                    .get(self.tx, &b.account())
                    .unwrap_or_default();
                generate_final_vote =
                    confirmation_height_info.height >= b.sideband().unwrap().height;
            }
        }

        // 5. Ledger by root
        if block.is_none() && !root.is_zero() {
            // Search for block root
            let successor = self.ledger.any().block_successor(self.tx, &(*root).into());

            // Search for account root
            if let Some(successor) = successor {
                let successor_block = self.ledger.any().get_block(self.tx, &successor).unwrap();
                block = Some(successor_block);

                // 5. Votes in cache for successor
                let mut find_successor_votes = self.local_votes.votes(root, &successor, false);
                if !find_successor_votes.is_empty() {
                    self.cached_votes.append(&mut find_successor_votes);
                    generate_vote = false;
                }
                // Confirmation status. Generate final votes for confirmed successor
                if let Some(b) = &block {
                    if generate_vote {
                        let confirmation_height_info = self
                            .ledger
                            .store
                            .confirmation_height
                            .get(self.tx, &b.account())
                            .unwrap();
                        generate_final_vote =
                            confirmation_height_info.height >= b.sideband().unwrap().height;
                    }
                }
            }
        }

        if let Some(block) = block {
            // Generate new vote
            if generate_vote {
                if generate_final_vote {
                    self.to_generate_final.push(Arc::new(block.clone()));
                } else {
                    self.to_generate.push(Arc::new(block.clone()));
                }
            }

            // Let the node know about the alternative block
            if block.hash() != *hash {
                let publish = Message::Publish(Publish::new(block));
                self.channel.send(
                    &publish,
                    None,
                    BufferDropPolicy::Limiter,
                    TrafficType::Generic,
                );
            }
        } else {
            self.stats.inc_dir(
                StatType::Requests,
                DetailType::RequestsUnknown,
                Direction::In,
            );
        }
    }

    pub fn deduplicate_votes(&mut self) {
        self.cached_votes
            .sort_by(|a, b| a.signature.cmp(&b.signature));
        self.cached_votes
            .dedup_by(|a, b| a.signature == b.signature);
    }

    pub fn get_result(self) -> AggregateResult {
        AggregateResult {
            remaining_normal: self.to_generate,
            remaining_final: self.to_generate_final,
        }
    }
}

pub(super) struct AggregateResult {
    pub remaining_normal: Vec<Arc<BlockEnum>>,
    pub remaining_final: Vec<Arc<BlockEnum>>,
}
