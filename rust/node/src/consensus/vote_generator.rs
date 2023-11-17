use std::{
    collections::VecDeque,
    net::SocketAddrV6,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use rsnano_core::{Account, BlockHash, Root};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::LmdbWriteTransaction;

use crate::{
    config::NetworkConstants,
    messages::ConfirmAck,
    representatives::RepresentativeRegister,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{ChannelEnum, InboundCallback, TcpChannels},
    utils::{AsyncRuntime, ProcessingQueue},
};

use super::{Vote, VoteBroadcaster, VoteProcessorQueue, VoteSpacing};

pub struct VoteGenerator {
    ledger: Arc<Ledger>,
    vote_generation_queue: ProcessingQueue<(Root, BlockHash)>,
    shared_state: Arc<SharedState>,
    thread: Option<JoinHandle<()>>,
}

impl VoteGenerator {
    pub fn new(
        ledger: Arc<Ledger>,
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
            condition: Condvar::new(),
            queues: Mutex::new(Queues::default()),
            is_final,
            stopped: AtomicBool::new(false),
            stats: Arc::clone(&stats),
            vote_broadcaster,
            spacing: VoteSpacing::new(voting_delay),
        });

        let shared_state_clone = Arc::clone(&shared_state);
        Self {
            ledger,
            shared_state,
            thread: None,
            vote_generation_queue: ProcessingQueue::new(
                stats,
                StatType::VoteGenerator,
                "Voting que".to_string(),
                1,         // single threaded
                1024 * 32, // max queue size
                1024 * 4,  // max batch size,
                Box::new(move |batch| {
                    shared_state_clone.process_batch(batch);
                }),
            ),
        }
    }

    pub fn start(&mut self) {
        let shared_state_clone = Arc::clone(&self.shared_state);
        self.thread = Some(
            thread::Builder::new()
                .name("voting".to_owned())
                .spawn(move || shared_state_clone.run())
                .unwrap(),
        );
        self.vote_generation_queue.start();
    }

    pub fn stop(&mut self) {
        self.vote_generation_queue.stop();
        self.shared_state.stopped.store(true, Ordering::SeqCst);
        self.shared_state.condition.notify_all();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }

    // TODO: fn add()

    pub fn should_vote(
        &self,
        txn: &mut LmdbWriteTransaction,
        root: &Root,
        hash: &BlockHash,
    ) -> bool {
        self.shared_state.should_vote(txn, root, hash)
    }
}

impl Drop for VoteGenerator {
    fn drop(&mut self) {
        self.stop()
    }
}

struct SharedState {
    ledger: Arc<Ledger>,
    is_final: bool,
    condition: Condvar,
    stopped: AtomicBool,
    queues: Mutex<Queues>,
    stats: Arc<Stats>,
    vote_broadcaster: VoteBroadcaster,
    spacing: VoteSpacing,
}

impl SharedState {
    fn run(&self) {
        let mut queues = self.queues.lock().unwrap();
        while !self.stopped.load(Ordering::SeqCst) {
            if queues.candidates.len() >= ConfirmAck::HASHES_MAX {
                queues = self.broadcast(queues);
            } else if let Some(_request) = queues.requests.pop_front() {
                //TODO
            } else {
                // TODO
            }
        }
    }

    fn broadcast<'a>(&'a self, mut queues: MutexGuard<'a, Queues>) -> MutexGuard<'a, Queues> {
        let mut hashes = Vec::with_capacity(ConfirmAck::HASHES_MAX);
        let mut roots = Vec::with_capacity(ConfirmAck::HASHES_MAX);
        while let Some((root, hash)) = queues.candidates.pop_front() {
            if !roots.contains(&root) {
                if self.spacing.votable(&root, &hash) {
                    roots.push(root);
                    hashes.push(hash);
                } else {
                    self.stats.inc(
                        StatType::VoteGenerator,
                        DetailType::GeneratorSpacing,
                        Direction::In,
                    );
                }
            }
            if hashes.len() == ConfirmAck::HASHES_MAX {
                break;
            }
        }

        if !hashes.is_empty() {
            drop(queues);
            // TODO
            //self.vote()
            queues = self.queues.lock().unwrap();
        }

        queues
    }

    fn vote<F>(&self, _hashes: &Vec<BlockHash>, _roots: &Vec<Root>, _f: F)
    where
        F: Fn(Arc<Vote>),
    {
        //let mut votes = Vec::new();
        // TODO
    }

    fn process_batch(&self, batch: VecDeque<(Root, BlockHash)>) {
        let mut candidates_new = VecDeque::new();
        {
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
                queues.candidates.len() >= ConfirmAck::HASHES_MAX
            };

            if should_notify {
                self.condition.notify_all();
            }
        }
    }

    fn should_vote(&self, txn: &mut LmdbWriteTransaction, root: &Root, hash: &BlockHash) -> bool {
        if self.is_final {
            match self.ledger.get_block(txn, hash) {
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
            match self.ledger.get_block(txn, hash) {
                Some(block) => self.ledger.dependents_confirmed(txn, &block),
                None => false,
            }
        }
    }
}

#[derive(Default)]
struct Queues {
    candidates: VecDeque<(Root, BlockHash)>,
    requests: VecDeque<(Vec<(Root, BlockHash)>, Arc<ChannelEnum>)>,
}
