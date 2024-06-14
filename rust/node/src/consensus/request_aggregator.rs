use super::{LocalVoteHistory, VoteGenerator, VoteRouter};
use crate::{
    config::NodeConfig,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, TrafficType},
    wallets::Wallets,
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockEnum, BlockHash, Root, Vote,
};
use rsnano_ledger::Ledger;
use rsnano_messages::{ConfirmAck, Message, Publish};
use std::{
    cmp::min,
    collections::{BTreeMap, HashMap, HashSet},
    mem::size_of,
    net::SocketAddrV6,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub struct RequestAggregatorConfig {
    threads: usize,
    max_queue: usize,
    batch_size: usize,
}

impl RequestAggregatorConfig {
    pub fn new(parallelism: usize) -> Self {
        Self {
            threads: min(parallelism, 4),
            max_queue: 128,
            batch_size: 16,
        }
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
    config: NodeConfig,
    stats: Arc<Stats>,
    generator: Arc<VoteGenerator>,
    final_generator: Arc<VoteGenerator>,
    local_votes: Arc<LocalVoteHistory>,
    ledger: Arc<Ledger>,
    wallets: Arc<Wallets>,
    vote_router: Arc<VoteRouter>,
    pub max_delay: Duration,
    small_delay: Duration,
    max_channel_requests: usize,
    request_aggregator_threads: usize,
    mutex: Mutex<RequestAggregatorData>,
    condition: Condvar,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl RequestAggregator {
    pub fn new(
        config: NodeConfig,
        stats: Arc<Stats>,
        generator: Arc<VoteGenerator>,
        final_generator: Arc<VoteGenerator>,
        local_votes: Arc<LocalVoteHistory>,
        ledger: Arc<Ledger>,
        wallets: Arc<Wallets>,
        vote_router: Arc<VoteRouter>,
        is_dev_network: bool,
    ) -> Self {
        Self {
            stats,
            generator,
            final_generator,
            local_votes,
            ledger,
            wallets,
            vote_router,
            max_delay: if is_dev_network {
                Duration::from_millis(50)
            } else {
                Duration::from_millis(300)
            },
            small_delay: if is_dev_network {
                Duration::from_millis(10)
            } else {
                Duration::from_millis(50)
            },
            max_channel_requests: config.max_queued_requests as usize,
            request_aggregator_threads: config.request_aggregator_threads as usize,
            config,
            condition: Condvar::new(),
            mutex: Mutex::new(RequestAggregatorData {
                requests: ChannelPoolContainer::default(),
                stopped: false,
                started: false,
            }),
            threads: Mutex::new(Vec::new()),
        }
    }

    /// Add a new request by channel for hashes hashes_roots
    /// TODO: This is badly implemented, will prematurely drop large vote requests
    pub fn add(&self, channel: Arc<ChannelEnum>, hashes_roots: &Vec<(BlockHash, Root)>) {
        debug_assert!(self.wallets.voting_reps_count() > 0);
        let mut error = true;
        let endpoint = channel.remote_endpoint();
        let mut guard = self.mutex.lock().unwrap();
        // Protecting from ever-increasing memory usage when request are consumed slower than generated
        // Reject request if the oldest request has not yet been processed after its deadline + a modest margin
        if guard.requests.is_empty()
            || (guard.requests.iter_by_deadline().next().unwrap().deadline + 2 * self.max_delay
                > Instant::now())
        {
            if !guard.requests.modify(&endpoint, |i| {
                // This extends the lifetime of the channel, which is acceptable up to max_delay
                i.channel = Arc::clone(&channel);
                error = !self.try_insert_hashes_roots(i, hashes_roots);
            }) {
                let mut pool = ChannelPool::new(channel);
                error = !self.try_insert_hashes_roots(&mut pool, hashes_roots);
                guard.requests.insert(pool);
            }
        }
        self.stats.inc(
            StatType::Aggregator,
            if !error {
                DetailType::AggregatorAccepted
            } else {
                DetailType::AggregatorDropped
            },
        );
    }

    pub fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        guard.started = true;
        drop(guard);
        self.condition.notify_all();
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            if !guard.requests.is_empty() {
                let front = guard.requests.iter_by_deadline().next().unwrap();
                if front.deadline < Instant::now() {
                    // Store the channel and requests for processing after erasing this pool

                    let endpoint = front.endpoint.clone();
                    let mut front = guard.requests.remove(&endpoint).unwrap();
                    drop(guard);
                    self.erase_duplicates(&mut front.hashes_roots);
                    let remaining = self.aggregate(&front.hashes_roots, &front.channel);
                    if !remaining.0.is_empty() {
                        // Generate votes for the remaining hashes
                        let generated = self
                            .generator
                            .generate(&remaining.0, Arc::clone(&front.channel));
                        self.stats.add_dir(
                            StatType::Requests,
                            DetailType::RequestsCannotVote,
                            Direction::In,
                            (remaining.0.len() - generated) as u64,
                        );
                    }
                    if !remaining.1.is_empty() {
                        // Generate final votes for the remaining hashes
                        let generated = self
                            .final_generator
                            .generate(&remaining.1, Arc::clone(&front.channel));
                        self.stats.add_dir(
                            StatType::Requests,
                            DetailType::RequestsCannotVote,
                            Direction::In,
                            (remaining.1.len() - generated) as u64,
                        );
                    }
                    guard = self.mutex.lock().unwrap();
                } else {
                    let deadline = front.deadline;
                    let duration = deadline.duration_since(Instant::now());
                    guard = self
                        .condition
                        .wait_timeout_while(guard, duration, |g| {
                            !g.stopped && deadline >= Instant::now()
                        })
                        .unwrap()
                        .0;
                }
            } else {
                guard = self
                    .condition
                    .wait_timeout_while(guard, self.small_delay, |g| {
                        !g.stopped && g.requests.is_empty()
                    })
                    .unwrap()
                    .0;
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

    /// Returns the number of currently queued request pools
    pub fn len(&self) -> usize {
        self.mutex.lock().unwrap().requests.len()
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

    fn try_insert_hashes_roots(
        &self,
        pool: &mut ChannelPool,
        hashes_roots: &Vec<(BlockHash, Root)>,
    ) -> bool {
        if pool.hashes_roots.len() + hashes_roots.len() <= self.max_channel_requests {
            let new_deadline = self.get_new_deadline(pool.start);
            pool.deadline = new_deadline;
            pool.insert_hashes_roots(hashes_roots);
            true
        } else {
            false
        }
    }

    fn get_new_deadline(&self, start: Instant) -> Instant {
        min(start + self.max_delay, Instant::now() + self.small_delay)
    }

    /// Aggregate requests and send cached votes to channel.
    /// Return the remaining hashes that need vote generation for each block for regular & final vote generators
    fn aggregate(
        &self,
        requests: &Vec<(BlockHash, Root)>,
        channel: &Arc<ChannelEnum>,
    ) -> (Vec<Arc<BlockEnum>>, Vec<Arc<BlockEnum>>) {
        let tx = self.ledger.read_txn();
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
                let final_vote_hashes = self.ledger.store.final_vote.get(&tx, *root);
                if !final_vote_hashes.is_empty() {
                    generate_final_vote = true;
                    block = self.ledger.any().get_block(&tx, &final_vote_hashes[0]);
                    // Allow same root vote
                    if let Some(b) = &block {
                        if final_vote_hashes.len() > 1 {
                            to_generate_final.push(Arc::new(b.clone()));
                            block = self.ledger.any().get_block(&tx, &final_vote_hashes[1]);
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
                    block = self.ledger.any().get_block(&tx, hash);
                    // Confirmation status. Generate final votes for confirmed
                    if let Some(b) = &block {
                        let confirmation_height_info = self
                            .ledger
                            .store
                            .confirmation_height
                            .get(&tx, &b.account())
                            .unwrap_or_default();
                        generate_final_vote =
                            confirmation_height_info.height >= b.sideband().unwrap().height;
                    }
                }

                // 5. Ledger by root
                if block.is_none() && !root.is_zero() {
                    // Search for block root
                    let successor = self.ledger.any().block_successor(&tx, &(*root).into());

                    // Search for account root
                    if let Some(successor) = successor {
                        let successor_block = self.ledger.any().get_block(&tx, &successor).unwrap();
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
                                    .get(&tx, &b.account())
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
        (to_generate, to_generate_final)
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "pools".to_string(),
                count: guard.requests.len(),
                sizeof_element: ChannelPoolContainer::ELEMENT_SIZE,
            })],
        )
    }
}

struct RequestAggregatorData {
    requests: ChannelPoolContainer,
    stopped: bool,
    started: bool,
}

/**
 * Holds a buffer of incoming requests from an endpoint.
 * Extends the lifetime of the corresponding channel. The channel is updated on a new request arriving from the same endpoint, such that only the newest channel is held
 */
struct ChannelPool {
    hashes_roots: Vec<(BlockHash, Root)>,
    channel: Arc<ChannelEnum>,
    endpoint: SocketAddrV6,
    start: Instant,
    deadline: Instant,
}

impl ChannelPool {
    pub fn new(channel: Arc<ChannelEnum>) -> Self {
        let now = Instant::now();
        Self {
            hashes_roots: Vec::new(),
            endpoint: channel.remote_endpoint(),
            channel,
            start: now,
            deadline: now,
        }
    }

    pub fn insert_hashes_roots(&mut self, hashes_roots: &Vec<(BlockHash, Root)>) {
        let old = self.hashes_roots.clone();
        self.hashes_roots
            .reserve(self.hashes_roots.len() + hashes_roots.len());
        self.hashes_roots.clear();
        self.hashes_roots.extend_from_slice(&hashes_roots);
        self.hashes_roots.extend_from_slice(&old);
    }
}

#[derive(Default)]
struct ChannelPoolContainer {
    by_endpoint: HashMap<SocketAddrV6, ChannelPool>,
    by_deadline: BTreeMap<Instant, Vec<SocketAddrV6>>,
}

impl ChannelPoolContainer {
    pub const ELEMENT_SIZE: usize =
        size_of::<ChannelPool>() + size_of::<SocketAddrV6>() * 2 + size_of::<Instant>();

    pub fn insert(&mut self, pool: ChannelPool) {
        let endpoint = pool.endpoint;
        let deadline = pool.deadline;

        if let Some(old) = self.by_endpoint.insert(pool.endpoint, pool) {
            self.remove_deadline(&old.endpoint, old.deadline);
        }

        self.by_deadline.entry(deadline).or_default().push(endpoint);
    }

    pub fn len(&self) -> usize {
        self.by_endpoint.len()
    }

    pub fn get(&self, addr: &SocketAddrV6) -> Option<&ChannelPool> {
        self.by_endpoint.get(addr)
    }

    pub fn modify(&mut self, endpoint: &SocketAddrV6, mut f: impl FnMut(&mut ChannelPool)) -> bool {
        if let Some(pool) = self.by_endpoint.get_mut(endpoint) {
            let old_deadline = pool.deadline;
            let endpoint = pool.endpoint;
            f(pool);
            let new_deadline = pool.deadline;
            if new_deadline != old_deadline {
                self.remove_deadline(&endpoint, old_deadline);
                self.by_deadline
                    .entry(new_deadline)
                    .or_default()
                    .push(endpoint);
            }
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, endpoint: &SocketAddrV6) -> Option<ChannelPool> {
        let pool = self.by_endpoint.remove(endpoint)?;
        self.remove_deadline(endpoint, pool.deadline);
        Some(pool)
    }

    pub fn iter_by_deadline(&self) -> impl Iterator<Item = &ChannelPool> {
        self.by_deadline
            .values()
            .flat_map(|addrs| addrs.iter().map(|a| self.by_endpoint.get(a).unwrap()))
    }

    pub fn is_empty(&self) -> bool {
        self.by_endpoint.is_empty()
    }

    fn remove_deadline(&mut self, endpoint: &SocketAddrV6, deadline: Instant) {
        let addrs = self.by_deadline.get_mut(&deadline).unwrap();
        if addrs.len() > 1 {
            addrs.retain(|i| i != endpoint);
        } else {
            self.by_deadline.remove(&deadline);
        }
    }
}

pub trait RequestAggregatorExt {
    fn start(&self);
}

impl RequestAggregatorExt for Arc<RequestAggregator> {
    fn start(&self) {
        {
            let mut guard = self.threads.lock().unwrap();
            for _ in 0..self.request_aggregator_threads {
                let self_l = Arc::clone(self);
                guard.push(
                    std::thread::Builder::new()
                        .name("Req aggregator".to_string())
                        .spawn(move || self_l.run())
                        .unwrap(),
                );
            }
        }

        let self_w = Arc::downgrade(self);
        self.generator
            .set_reply_action(Box::new(move |vote, channel| {
                if let Some(self_l) = self_w.upgrade() {
                    self_l.reply_action(vote, channel);
                }
            }));

        let self_w = Arc::downgrade(self);
        self.final_generator
            .set_reply_action(Box::new(move |vote, channel| {
                if let Some(self_l) = self_w.upgrade() {
                    self_l.reply_action(vote, channel);
                }
            }));

        let guard = self.mutex.lock().unwrap();
        drop(self.condition.wait_while(guard, |g| !g.started).unwrap());
    }
}
