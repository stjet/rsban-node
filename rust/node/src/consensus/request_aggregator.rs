use super::{LocalVoteHistory, VoteGenerators, VoteRouter};
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, FairQueue, Origin, TrafficType},
};
use rsnano_core::{
    utils::{ContainerInfoComponent, TomlWriter},
    BlockEnum, BlockHash, NoValue, Root, Vote,
};
use rsnano_ledger::Ledger;
use rsnano_messages::{ConfirmAck, Message, Publish};
use rsnano_store_lmdb::{LmdbReadTransaction, Transaction};
use std::{
    cmp::min,
    collections::HashSet,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
};
use tracing::trace;

#[derive(Debug, Clone)]
pub struct RequestAggregatorConfig {
    pub threads: usize,
    pub max_queue: usize,
    pub batch_size: usize,
}

impl RequestAggregatorConfig {
    pub fn new(parallelism: usize) -> Self {
        Self {
            threads: min(parallelism, 4),
            max_queue: 128,
            batch_size: 16,
        }
    }

    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_usize(
            "max_queue",
            self.max_queue,
            "Maximum number of queued requests per peer. \ntype:uint64",
        )?;
        toml.put_usize(
            "threads",
            self.threads,
            "Number of threads for request processing. \ntype:uint64",
        )?;
        toml.put_usize(
            "batch_size",
            self.batch_size,
            "Number of requests to process in a single batch. \ntype:uint64",
        )
    }
}

/**
 * Pools together confirmation requests, separately for each endpoint.
 * Requests are added from network messages, and aggregated to minimize bandwidth and vote generation. Example:
 * * Two votes are cached, one for hashes {1,2,3} and another for hashes {4,5,6}
 * * A request arrives for hashes {1,4,5}. Another request arrives soon afterwards for hashes {2,3,6}
 * * The aggregator will reply with the two cached votes
 * Votes are generated for uncached hashes.
 */
pub struct RequestAggregator {
    config: RequestAggregatorConfig,
    stats: Arc<Stats>,
    vote_generators: Arc<VoteGenerators>,
    local_votes: Arc<LocalVoteHistory>,
    ledger: Arc<Ledger>,
    vote_router: Arc<VoteRouter>,
    mutex: Mutex<RequestAggregatorData>,
    condition: Condvar,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl RequestAggregator {
    pub fn new(
        config: RequestAggregatorConfig,
        stats: Arc<Stats>,
        vote_generators: Arc<VoteGenerators>,
        local_votes: Arc<LocalVoteHistory>,
        ledger: Arc<Ledger>,
        vote_router: Arc<VoteRouter>,
    ) -> Self {
        let max_queue = config.max_queue;
        Self {
            stats,
            vote_generators,
            local_votes,
            ledger,
            vote_router,
            config,
            condition: Condvar::new(),
            mutex: Mutex::new(RequestAggregatorData {
                queue: FairQueue::new(Box::new(move |_| max_queue), Box::new(|_| 1)),
                stopped: false,
            }),
            threads: Mutex::new(Vec::new()),
        }
    }

    pub fn request(&self, request: RequestType, channel: Arc<ChannelEnum>) -> bool {
        // This should be checked before calling request
        debug_assert!(!request.is_empty());
        let request_len = request.len();

        let added = {
            self.mutex
                .lock()
                .unwrap()
                .queue
                .push((request, channel.clone()), Origin::new(NoValue {}, channel))
        };

        if added {
            self.stats
                .inc(StatType::RequestAggregator, DetailType::Request);
            self.stats.add(
                StatType::RequestAggregator,
                DetailType::RequestHashes,
                request_len as u64,
            );
            self.condition.notify_one();
        } else {
            self.stats
                .inc(StatType::RequestAggregator, DetailType::Overfill);
            self.stats.add(
                StatType::RequestAggregator,
                DetailType::OverfillHashes,
                request_len as u64,
            );
        }

        // TODO: This stat is for compatibility with existing tests and is in principle unnecessary
        self.stats.inc(
            StatType::Aggregator,
            if added {
                DetailType::AggregatorAccepted
            } else {
                DetailType::AggregatorDropped
            },
        );

        added
    }

    pub fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            trace!("loop");

            if !guard.queue.is_empty() {
                guard = self.run_batch(guard);
            } else {
                guard = self
                    .condition
                    .wait_while(guard, |g| !g.stopped && g.queue.is_empty())
                    .unwrap();
            }
        }
    }

    pub fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        let mut threads = Vec::new();
        {
            let mut guard = self.threads.lock().unwrap();
            std::mem::swap(&mut threads, &mut *guard);
        }
        for thread in threads {
            thread.join().unwrap();
        }
    }

    fn run_batch<'a>(
        &'a self,
        mut state: MutexGuard<'a, RequestAggregatorData>,
    ) -> MutexGuard<'a, RequestAggregatorData> {
        let batch = state.queue.next_batch(self.config.batch_size);
        drop(state);

        let mut tx = self.ledger.read_txn();

        for ((request, channel), _) in &batch {
            tx.refresh_if_needed();

            if !channel.max(TrafficType::Generic) {
                self.process(&tx, request, channel);
            } else {
                self.stats.inc_dir(
                    StatType::RequestAggregator,
                    DetailType::ChannelFull,
                    Direction::Out,
                );
            }
        }

        self.mutex.lock().unwrap()
    }

    fn process(&self, tx: &LmdbReadTransaction, request: &RequestType, channel: &Arc<ChannelEnum>) {
        let remaining = self.aggregate(tx, request, channel);

        if !remaining.remaining_normal.is_empty() {
            // Generate votes for the remaining hashes
            let generated = self
                .vote_generators
                .generate_non_final_votes(&remaining.remaining_normal, channel.clone());
            self.stats.add_dir(
                StatType::Requests,
                DetailType::RequestsCannotVote,
                Direction::In,
                (remaining.remaining_normal.len() - generated) as u64,
            );
        }

        if !remaining.remaining_final.is_empty() {
            // Generate final votes for the remaining hashes
            let generated = self
                .vote_generators
                .generate_final_votes(&remaining.remaining_final, channel.clone());
            self.stats.add_dir(
                StatType::Requests,
                DetailType::RequestsCannotVote,
                Direction::In,
                (remaining.remaining_final.len() - generated) as u64,
            );
        }
    }

    /// Returns the number of currently queued request pools
    pub fn len(&self) -> usize {
        self.mutex.lock().unwrap().queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn reply_action(&self, vote: &Arc<Vote>, channel: &ChannelEnum) {
        let confirm = Message::ConfirmAck(ConfirmAck::new((**vote).clone()));
        channel.send(
            &confirm,
            None,
            BufferDropPolicy::Limiter,
            TrafficType::Generic,
        );
    }

    fn erase_duplicates(&self, requests: &mut Vec<(BlockHash, Root)>) {
        requests.sort_by(|a, b| a.0.cmp(&b.0));
        requests.dedup_by_key(|i| i.0);
    }

    /// Aggregate requests and send cached votes to channel.
    /// Return the remaining hashes that need vote generation for each block for regular & final vote generators
    fn aggregate(
        &self,
        tx: &LmdbReadTransaction,
        requests: &RequestType,
        channel: &Arc<ChannelEnum>,
    ) -> AggregateResult {
        let mut to_generate: Vec<Arc<BlockEnum>> = Vec::new();
        let mut to_generate_final: Vec<Arc<BlockEnum>> = Vec::new();
        let mut cached_votes: Vec<Arc<Vote>> = Vec::new();
        let mut cached_hashes: HashSet<BlockHash> = HashSet::new();
        for (hash, root) in requests {
            // 0. Hashes already sent
            if cached_hashes.contains(hash) {
                continue;
            }

            // 1. Votes in cache
            let find_votes = self.local_votes.votes(root, hash, false);
            if !find_votes.is_empty() {
                for found_vote in find_votes {
                    for found_hash in &found_vote.hashes {
                        cached_hashes.insert(*found_hash);
                    }
                    cached_votes.push(found_vote);
                }
            } else {
                let mut generate_vote = true;
                let mut generate_final_vote = false;
                let mut block = None;

                // 2. Final votes
                let final_vote_hashes = self.ledger.store.final_vote.get(tx, *root);
                if !final_vote_hashes.is_empty() {
                    generate_final_vote = true;
                    block = self.ledger.any().get_block(tx, &final_vote_hashes[0]);
                    // Allow same root vote
                    if let Some(b) = &block {
                        if final_vote_hashes.len() > 1 {
                            to_generate_final.push(Arc::new(b.clone()));
                            block = self.ledger.any().get_block(tx, &final_vote_hashes[1]);
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
                    block = self.ledger.any().get_block(tx, hash);
                    // Confirmation status. Generate final votes for confirmed
                    if let Some(b) = &block {
                        let confirmation_height_info = self
                            .ledger
                            .store
                            .confirmation_height
                            .get(tx, &b.account())
                            .unwrap_or_default();
                        generate_final_vote =
                            confirmation_height_info.height >= b.sideband().unwrap().height;
                    }
                }

                // 5. Ledger by root
                if block.is_none() && !root.is_zero() {
                    // Search for block root
                    let successor = self.ledger.any().block_successor(tx, &(*root).into());

                    // Search for account root
                    if let Some(successor) = successor {
                        let successor_block = self.ledger.any().get_block(tx, &successor).unwrap();
                        block = Some(successor_block);

                        // 5. Votes in cache for successor
                        let mut find_successor_votes =
                            self.local_votes.votes(root, &successor, false);
                        if !find_successor_votes.is_empty() {
                            cached_votes.append(&mut find_successor_votes);
                            generate_vote = false;
                        }
                        // Confirmation status. Generate final votes for confirmed successor
                        if let Some(b) = &block {
                            if generate_vote {
                                let confirmation_height_info = self
                                    .ledger
                                    .store
                                    .confirmation_height
                                    .get(tx, &b.account())
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
                            to_generate_final.push(Arc::new(block.clone()));
                        } else {
                            to_generate.push(Arc::new(block.clone()));
                        }
                    }

                    // Let the node know about the alternative block
                    if block.hash() != *hash {
                        let publish = Message::Publish(Publish::new(block));
                        channel.send(
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
        }

        // Unique votes
        cached_votes.sort_by(|a, b| a.signature.cmp(&b.signature));
        cached_votes.dedup_by(|a, b| a.signature == b.signature);

        let cached_votes_len = cached_votes.len() as u64;
        for vote in cached_votes {
            self.reply_action(&vote, channel);
        }

        self.stats.add_dir(
            StatType::Requests,
            DetailType::RequestsCachedHashes,
            Direction::In,
            cached_hashes.len() as u64,
        );

        self.stats.add_dir(
            StatType::Requests,
            DetailType::RequestsCachedVotes,
            Direction::In,
            cached_votes_len,
        );

        AggregateResult {
            remaining_normal: to_generate,
            remaining_final: to_generate_final,
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![guard.queue.collect_container_info("queue")],
        )
    }
}

impl Drop for RequestAggregator {
    fn drop(&mut self) {
        debug_assert!(self.threads.lock().unwrap().is_empty())
    }
}

type RequestType = Vec<(BlockHash, Root)>;
type ValueType = (RequestType, Arc<ChannelEnum>);

struct RequestAggregatorData {
    queue: FairQueue<ValueType, NoValue>,
    stopped: bool,
}

pub trait RequestAggregatorExt {
    fn start(&self);
}

impl RequestAggregatorExt for Arc<RequestAggregator> {
    fn start(&self) {
        let self_w = Arc::downgrade(self);
        self.vote_generators
            .set_reply_action(Arc::new(move |vote, channel| {
                if let Some(self_l) = self_w.upgrade() {
                    self_l.reply_action(vote, channel);
                }
            }));

        let mut guard = self.threads.lock().unwrap();
        for _ in 0..self.config.threads {
            let self_l = Arc::clone(self);
            guard.push(
                std::thread::Builder::new()
                    .name("Req aggregator".to_string())
                    .spawn(move || self_l.run())
                    .unwrap(),
            );
        }
    }
}

struct AggregateResult {
    remaining_normal: Vec<Arc<BlockEnum>>,
    remaining_final: Vec<Arc<BlockEnum>>,
}
