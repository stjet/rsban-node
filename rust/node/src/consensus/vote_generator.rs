use std::{
    collections::VecDeque,
    mem::size_of,
    net::SocketAddrV6,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use rsnano_core::{
    utils::{milliseconds_since_epoch, ContainerInfo, ContainerInfoComponent},
    Account, BlockEnum, BlockHash, Root, Vote,
};
use rsnano_ledger::{Ledger, Writer};
use rsnano_store_lmdb::LmdbWriteTransaction;
use tracing::trace;

use crate::{
    config::NetworkConstants,
    representatives::RepresentativeRegister,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{ChannelEnum, InboundCallback, TcpChannels},
    utils::{AsyncRuntime, ProcessingQueue},
    wallets::Wallets,
};

use super::{LocalVoteHistory, VoteBroadcaster, VoteProcessorQueue, VoteSpacing};

pub struct VoteGenerator {
    ledger: Arc<Ledger>,
    vote_generation_queue: ProcessingQueue<(Root, BlockHash)>,
    shared_state: Arc<SharedState>,
    thread: Mutex<Option<JoinHandle<()>>>,
    stats: Arc<Stats>,
}

impl VoteGenerator {
    const MAX_REQUESTS: usize = 2048;
    const MAX_HASHES: usize = 12;

    pub fn new(
        ledger: Arc<Ledger>,
        wallets: Arc<Wallets>,
        history: Arc<LocalVoteHistory>,
        is_final: bool,
        stats: Arc<Stats>,
        representative_register: Arc<Mutex<RepresentativeRegister>>,
        tcp_channels: Arc<TcpChannels>,
        vote_processor_queue: Arc<VoteProcessorQueue>,
        network_constants: NetworkConstants,
        async_rt: Arc<AsyncRuntime>,
        node_id: Account,
        local_endpoint: SocketAddrV6,
        inbound: InboundCallback,
        voting_delay: Duration,
        vote_generator_delay: Duration,
        vote_generator_threshold: usize,
    ) -> Self {
        let vote_broadcaster = VoteBroadcaster {
            representative_register,
            tcp_channels,
            vote_processor_queue,
            network_constants,
            stats: Arc::clone(&stats),
            async_rt,
            node_id,
            local_endpoint,
            inbound,
        };

        let shared_state = Arc::new(SharedState {
            ledger: Arc::clone(&ledger),
            history,
            wallets,
            condition: Condvar::new(),
            queues: Mutex::new(Queues::default()),
            is_final,
            stopped: AtomicBool::new(false),
            stats: Arc::clone(&stats),
            vote_broadcaster,
            spacing: Mutex::new(VoteSpacing::new(voting_delay)),
            vote_generator_delay,
            vote_generator_threshold,
            reply_action: Mutex::new(None),
        });

        let shared_state_clone = Arc::clone(&shared_state);
        Self {
            ledger,
            shared_state,
            thread: Mutex::new(None),
            vote_generation_queue: ProcessingQueue::new(
                Arc::clone(&stats),
                StatType::VoteGenerator,
                "Voting que".to_string(),
                1,         // single threaded
                1024 * 32, // max queue size
                1024 * 4,  // max batch size,
                Box::new(move |batch| {
                    shared_state_clone.process_batch(batch);
                }),
            ),
            stats,
        }
    }

    pub fn set_reply_action(
        &self,
        action: Box<dyn Fn(&Arc<Vote>, &Arc<ChannelEnum>) + Send + Sync>,
    ) {
        let mut guard = self.shared_state.reply_action.lock().unwrap();
        *guard = Some(action);
    }

    pub fn start(&self) {
        let shared_state_clone = Arc::clone(&self.shared_state);
        *self.thread.lock().unwrap() = Some(
            thread::Builder::new()
                .name("voting".to_owned())
                .spawn(move || shared_state_clone.run())
                .unwrap(),
        );
        self.vote_generation_queue.start();
    }

    pub fn stop(&self) {
        self.vote_generation_queue.stop();
        self.shared_state.stopped.store(true, Ordering::SeqCst);
        self.shared_state.condition.notify_all();
        if let Some(thread) = self.thread.lock().unwrap().take() {
            thread.join().unwrap();
        }
    }

    /// Queue items for vote generation, or broadcast votes already in cache
    pub fn add(&self, root: &Root, hash: &BlockHash) {
        self.vote_generation_queue.add((*root, *hash));
    }

    /// Queue blocks for vote generation, returning the number of successful candidates.
    pub fn generate(&self, blocks: &[Arc<BlockEnum>], channel: Arc<ChannelEnum>) -> usize {
        let req_candidates = {
            let txn = self.ledger.read_txn();
            blocks
                .iter()
                .filter_map(|i| {
                    if self.ledger.dependents_confirmed(&txn, i) {
                        Some((i.root(), i.hash()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };

        let result = req_candidates.len();
        let mut guard = self.shared_state.queues.lock().unwrap();
        guard.requests.push_back((req_candidates, channel));
        while guard.requests.len() > Self::MAX_REQUESTS {
            // On a large queue of requests, erase the oldest one
            guard.requests.pop_front();
            self.stats.inc(
                StatType::VoteGenerator,
                DetailType::GeneratorRepliesDiscarded,
            );
        }

        result
    }

    /// Check if block is eligible for vote generation
    /// @param transaction : needs `tables::final_votes` lock
    /// @return: Should vote
    pub fn should_vote(
        &self,
        txn: &mut LmdbWriteTransaction,
        root: &Root,
        hash: &BlockHash,
    ) -> bool {
        self.shared_state.should_vote(txn, root, hash)
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let candidates_count;
        let requests_count;
        {
            let guard = self.shared_state.queues.lock().unwrap();
            candidates_count = guard.candidates.len();
            requests_count = guard.requests.len();
        }
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "candidates".to_string(),
                    count: candidates_count,
                    sizeof_element: size_of::<Root>() + size_of::<BlockHash>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "requests".to_string(),
                    count: requests_count,
                    sizeof_element: size_of::<Arc<ChannelEnum>>()
                        + size_of::<Vec<(Root, BlockHash)>>(),
                }),
            ],
        )
    }
}

impl Drop for VoteGenerator {
    fn drop(&mut self) {
        self.stop()
    }
}

struct SharedState {
    ledger: Arc<Ledger>,
    wallets: Arc<Wallets>,
    history: Arc<LocalVoteHistory>,
    is_final: bool,
    condition: Condvar,
    stopped: AtomicBool,
    queues: Mutex<Queues>,
    stats: Arc<Stats>,
    vote_broadcaster: VoteBroadcaster,
    spacing: Mutex<VoteSpacing>,
    vote_generator_delay: Duration,
    vote_generator_threshold: usize,
    reply_action: Mutex<Option<Box<dyn Fn(&Arc<Vote>, &Arc<ChannelEnum>) + Send + Sync>>>,
}

impl SharedState {
    fn run(&self) {
        let mut queues = self.queues.lock().unwrap();
        while !self.stopped.load(Ordering::SeqCst) {
            if queues.candidates.len() >= VoteGenerator::MAX_HASHES {
                queues = self.broadcast(queues);
            } else if let Some(request) = queues.requests.pop_front() {
                drop(queues);
                self.reply(request);
                queues = self.queues.lock().unwrap();
            } else {
                queues = self
                    .condition
                    .wait_timeout_while(queues, self.vote_generator_delay, |lk| {
                        lk.candidates.len() < VoteGenerator::MAX_HASHES
                    })
                    .unwrap()
                    .0;

                if queues.candidates.len() >= self.vote_generator_threshold
                    && queues.candidates.len() < VoteGenerator::MAX_HASHES
                {
                    queues = self
                        .condition
                        .wait_timeout_while(queues, self.vote_generator_delay, |lk| {
                            lk.candidates.len() < VoteGenerator::MAX_HASHES
                        })
                        .unwrap()
                        .0;
                }

                if !queues.candidates.is_empty() {
                    queues = self.broadcast(queues);
                }
            }
        }
    }

    fn broadcast<'a>(&'a self, mut queues: MutexGuard<'a, Queues>) -> MutexGuard<'a, Queues> {
        let mut hashes = Vec::with_capacity(VoteGenerator::MAX_HASHES);
        let mut roots = Vec::with_capacity(VoteGenerator::MAX_HASHES);
        {
            let spacing = self.spacing.lock().unwrap();
            while let Some((root, hash)) = queues.candidates.pop_front() {
                if !roots.contains(&root) {
                    if spacing.votable(&root, &hash) {
                        roots.push(root);
                        hashes.push(hash);
                    } else {
                        self.stats
                            .inc(StatType::VoteGenerator, DetailType::GeneratorSpacing);
                    }
                }
                if hashes.len() == VoteGenerator::MAX_HASHES {
                    break;
                }
            }
        }

        if !hashes.is_empty() {
            drop(queues);
            self.vote(&hashes, &roots, |vote| {
                self.vote_broadcaster.broadcast(vote);
                self.stats
                    .inc(StatType::VoteGenerator, DetailType::GeneratorBroadcasts);
            });
            queues = self.queues.lock().unwrap();
        }

        queues
    }

    fn vote<F>(&self, hashes: &Vec<BlockHash>, roots: &Vec<Root>, action: F)
    where
        F: Fn(Arc<Vote>),
    {
        debug_assert_eq!(hashes.len(), roots.len());
        let mut votes = Vec::new();
        self.wallets.foreach_representative(|pub_key, priv_key| {
            let timestamp = if self.is_final {
                Vote::TIMESTAMP_MAX
            } else {
                milliseconds_since_epoch()
            };
            let duration = if self.is_final {
                Vote::DURATION_MAX
            } else {
                0x9 /*8192ms*/
            };
            votes.push(Arc::new(Vote::new(
                *pub_key,
                priv_key,
                timestamp,
                duration,
                hashes.clone(),
            )));
        });

        for vote in votes {
            {
                let mut spacing = self.spacing.lock().unwrap();
                for i in 0..hashes.len() {
                    self.history.add(&roots[i], &hashes[i], &vote);
                    spacing.flag(&roots[i], &hashes[i]);
                }
            }
            action(vote);
        }
    }

    fn reply(&self, request: (Vec<(Root, BlockHash)>, Arc<ChannelEnum>)) {
        let mut i = request.0.iter().peekable();
        while i.peek().is_some() && !self.stopped.load(Ordering::SeqCst) {
            let mut hashes = Vec::with_capacity(VoteGenerator::MAX_HASHES);
            let mut roots = Vec::with_capacity(VoteGenerator::MAX_HASHES);
            {
                let spacing = self.spacing.lock().unwrap();
                while hashes.len() < VoteGenerator::MAX_HASHES {
                    let Some((root, hash)) = i.next() else {
                        break;
                    };
                    if !roots.contains(root) {
                        if spacing.votable(root, hash) {
                            roots.push(*root);
                            hashes.push(*hash);
                        } else {
                            self.stats
                                .inc(StatType::VoteGenerator, DetailType::GeneratorSpacing);
                        }
                    }
                }
            }
            if !hashes.is_empty() {
                self.stats.add(
                    StatType::Requests,
                    DetailType::RequestsGeneratedHashes,
                    Direction::In,
                    hashes.len() as u64,
                    false,
                );
                self.vote(&hashes, &roots, |vote| {
                    let action = self.reply_action.lock().unwrap();
                    if let Some(action) = action.deref() {
                        (action)(&vote, &request.1);
                    }
                    self.stats.inc_dir(
                        StatType::Requests,
                        DetailType::RequestsGeneratedVotes,
                        Direction::In,
                    );
                });
            }
        }
        self.stats
            .inc(StatType::VoteGenerator, DetailType::GeneratorReplies);
    }

    fn process_batch(&self, batch: VecDeque<(Root, BlockHash)>) {
        let mut candidates_new = VecDeque::new();
        {
            let writer = if self.is_final {
                Writer::VotingFinal
            } else {
                Writer::Voting
            };
            let _guard = self.ledger.write_queue.wait(writer);
            let mut txn = self.ledger.rw_txn();
            for (root, hash) in batch {
                if self.should_vote(&mut txn, &root, &hash) {
                    candidates_new.push_back((root, hash))
                }
            }
        }

        if !candidates_new.is_empty() {
            let should_notify = {
                let mut queues = self.queues.lock().unwrap();
                queues.candidates.extend(candidates_new);
                queues.candidates.len() >= VoteGenerator::MAX_HASHES
            };

            if should_notify {
                self.condition.notify_all();
            }
        }
    }

    fn should_vote(&self, txn: &mut LmdbWriteTransaction, root: &Root, hash: &BlockHash) -> bool {
        let block = self.ledger.any().get_block(txn, hash);
        let should_vote = if self.is_final {
            match &block {
                Some(block) => {
                    debug_assert!(block.root() == *root);
                    self.ledger.dependents_confirmed(txn, &block)
                        && self
                            .ledger
                            .store
                            .final_vote
                            .put(txn, &block.qualified_root(), hash)
                }
                None => false,
            }
        } else {
            match &block {
                Some(block) => self.ledger.dependents_confirmed(txn, &block),
                None => false,
            }
        };

        trace!(
            should_vote,
            is_final = self.is_final,
            block = %block.map(|b| b.hash()).unwrap_or_default(),
            "Should vote"
        );

        should_vote
    }
}

#[derive(Default)]
struct Queues {
    candidates: VecDeque<(Root, BlockHash)>,
    requests: VecDeque<(Vec<(Root, BlockHash)>, Arc<ChannelEnum>)>,
}
