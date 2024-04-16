use super::{
    Election, ElectionBehavior, ElectionData, ElectionState, ElectionStatus, LocalVoteHistory,
    RecentlyConfirmedCache, VoteCache, VoteGenerator,
};
use crate::{
    block_processing::BlockProcessor,
    cementation::ConfirmingSet,
    config::NodeConfig,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, TcpChannels},
    utils::ThreadPool,
    wallets::Wallets,
    NetworkParams, OnlineReps,
};
use rsnano_core::{utils::MemoryStream, Amount, BlockEnum, BlockHash, QualifiedRoot, Vote};
use rsnano_ledger::Ledger;
use rsnano_messages::{Message, Publish};
use std::{
    cmp::max,
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::{atomic::Ordering, Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};
use tracing::trace;

const ELECTION_MAX_BLOCKS: usize = 10;

pub struct ActiveTransactions {
    pub mutex: Mutex<ActiveTransactionsData>,
    pub condition: Condvar,
    network: NetworkParams,
    pub online_reps: Arc<Mutex<OnlineReps>>,
    wallets: Arc<Wallets>,
    pub election_winner_details: Mutex<HashMap<BlockHash, Arc<Election>>>,
    config: NodeConfig,
    ledger: Arc<Ledger>,
    confirming_set: Arc<ConfirmingSet>,
    workers: Arc<dyn ThreadPool>,
    pub recently_confirmed: Arc<RecentlyConfirmedCache>,
    history: Arc<LocalVoteHistory>,
    block_processor: Arc<BlockProcessor>,
    generator: Arc<VoteGenerator>,
    final_generator: Arc<VoteGenerator>,
    tcp_channels: Arc<TcpChannels>,
    pub vacancy_update: Mutex<Box<dyn Fn() + Send + Sync>>,
    vote_cache: Arc<Mutex<VoteCache>>,
    stats: Arc<Stats>,
    active_stopped_observer: Box<dyn Fn(BlockHash) + Send + Sync>,
}

impl ActiveTransactions {
    pub fn new(
        network: NetworkParams,
        online_reps: Arc<Mutex<OnlineReps>>,
        wallets: Arc<Wallets>,
        config: NodeConfig,
        ledger: Arc<Ledger>,
        confirming_set: Arc<ConfirmingSet>,
        workers: Arc<dyn ThreadPool>,
        history: Arc<LocalVoteHistory>,
        block_processor: Arc<BlockProcessor>,
        generator: Arc<VoteGenerator>,
        final_generator: Arc<VoteGenerator>,
        tcp_channels: Arc<TcpChannels>,
        vote_cache: Arc<Mutex<VoteCache>>,
        stats: Arc<Stats>,
        active_stopped_observer: Box<dyn Fn(BlockHash) + Send + Sync>,
    ) -> Self {
        Self {
            mutex: Mutex::new(ActiveTransactionsData {
                roots: OrderedRoots::default(),
                stopped: false,
                normal_count: 0,
                hinted_count: 0,
                optimistic_count: 0,
                blocks: HashMap::new(),
            }),
            condition: Condvar::new(),
            network,
            online_reps,
            wallets,
            election_winner_details: Mutex::new(HashMap::new()),
            config,
            ledger,
            confirming_set,
            workers,
            recently_confirmed: Arc::new(RecentlyConfirmedCache::new(65536)),
            history,
            block_processor,
            generator,
            final_generator,
            tcp_channels,
            vacancy_update: Mutex::new(Box::new(|| {})),
            vote_cache,
            stats,
            active_stopped_observer,
        }
    }

    pub fn request_loop<'a>(
        &self,
        stamp: Instant,
        guard: MutexGuard<'a, ActiveTransactionsData>,
    ) -> MutexGuard<'a, ActiveTransactionsData> {
        if !guard.stopped {
            let loop_interval =
                Duration::from_millis(self.network.network.aec_loop_interval_ms as u64);
            let min_sleep = loop_interval / 2;

            let wait_duration = max(
                min_sleep,
                (stamp + loop_interval).saturating_duration_since(Instant::now()),
            );

            self.condition
                .wait_timeout_while(guard, wait_duration, |data| !data.stopped)
                .unwrap()
                .0
        } else {
            guard
        }
    }

    /// Calculates minimum time delay between subsequent votes when processing non-final votes
    pub fn cooldown_time(&self, weight: Amount) -> Duration {
        let online_stake = { self.online_reps.lock().unwrap().trended() };
        if weight > online_stake / 20 {
            // Reps with more than 5% weight
            Duration::from_secs(1)
        } else if weight > online_stake / 100 {
            // Reps with more than 1% weight
            Duration::from_secs(5)
        } else {
            // The rest of smaller reps
            Duration::from_secs(15)
        }
    }

    pub fn remove_election_winner_details(&self, hash: &BlockHash) -> Option<Arc<Election>> {
        let mut guard = self.election_winner_details.lock().unwrap();
        guard.remove(hash)
    }

    pub fn tally_impl(
        &self,
        guard: &mut MutexGuard<ElectionData>,
    ) -> BTreeMap<TallyKey, Arc<BlockEnum>> {
        let mut block_weights: HashMap<BlockHash, Amount> = HashMap::new();
        let mut final_weights: HashMap<BlockHash, Amount> = HashMap::new();
        for (account, info) in &guard.last_votes {
            let rep_weight = self.ledger.weight(account);
            *block_weights.entry(info.hash).or_default() += rep_weight;
            if info.timestamp == u64::MAX {
                *final_weights.entry(info.hash).or_default() += rep_weight;
            }
        }
        guard.last_tally.clear();
        for (&hash, &weight) in &block_weights {
            guard.last_tally.insert(hash, weight);
        }
        let mut result = BTreeMap::new();
        for (hash, weight) in &block_weights {
            if let Some(block) = guard.last_blocks.get(hash) {
                result.insert(TallyKey(*weight), Arc::clone(block));
            }
        }
        // Calculate final votes sum for winner
        if !final_weights.is_empty() && !result.is_empty() {
            let winner_hash = result.first_key_value().unwrap().1.hash();
            if let Some(final_weight) = final_weights.get(&winner_hash) {
                guard.final_weight = *final_weight;
            }
        }
        result
    }

    pub fn remove_block(&self, election_guard: &mut MutexGuard<ElectionData>, hash: &BlockHash) {
        if election_guard.status.winner.as_ref().unwrap().hash() != *hash {
            if let Some(existing) = election_guard.last_blocks.remove(hash) {
                election_guard.last_votes.retain(|_, v| v.hash != *hash);
                self.clear_publish_filter(&existing);
            }
        }
    }

    fn clear_publish_filter(&self, block: &BlockEnum) {
        let mut buf = MemoryStream::new();
        block.serialize_without_block_type(&mut buf);
        self.tcp_channels.publish_filter.clear_bytes(buf.as_bytes());
    }

    pub fn remove_votes(
        &self,
        election: &Election,
        guard: &mut MutexGuard<ElectionData>,
        hash: &BlockHash,
    ) {
        if self.config.enable_voting && self.wallets.voting_reps_count() > 0 {
            // Remove votes from election
            let list_generated_votes = self.history.votes(&election.root, hash, false);
            for vote in list_generated_votes {
                guard.last_votes.remove(&vote.voting_account);
            }
            // Clear votes cache
            self.history.erase(&election.root);
        }
    }

    pub fn have_quorum(&self, tally: &BTreeMap<TallyKey, Arc<BlockEnum>>) -> bool {
        let mut it = tally.keys();
        let first = it.next().map(|i| i.amount()).unwrap_or_default();
        let second = it.next().map(|i| i.amount()).unwrap_or_default();
        let delta = self.online_reps.lock().unwrap().delta();
        first - second >= delta
    }

    /// Maximum number of elections that should be present in this container
    /// NOTE: This is only a soft limit, it is possible for this container to exceed this count
    pub fn limit(&self, behavior: ElectionBehavior) -> usize {
        match behavior {
            ElectionBehavior::Normal => self.config.active_elections_size,
            ElectionBehavior::Hinted => {
                self.config.active_elections_hinted_limit_percentage
                    * self.config.active_elections_size
                    / 100
            }
            ElectionBehavior::Optimistic => {
                self.config.active_elections_optimistic_limit_percentage
                    * self.config.active_elections_size
                    / 100
            }
        }
    }

    /// How many election slots are available for specified election type
    pub fn vacancy(&self, behavior: ElectionBehavior) -> i64 {
        let guard = self.mutex.lock().unwrap();
        match behavior {
            ElectionBehavior::Normal => {
                self.limit(ElectionBehavior::Normal) as i64 - guard.roots.len() as i64
            }
            ElectionBehavior::Hinted | ElectionBehavior::Optimistic => {
                self.limit(behavior) as i64 - guard.count_by_behavior(behavior) as i64
            }
        }
    }

    pub fn clear(&self) {
        {
            let mut guard = self.mutex.lock().unwrap();
            guard.blocks.clear();
            guard.roots.clear();
        }
        (self.vacancy_update.lock().unwrap())()
    }

    pub fn confirmed_locked(&self, guard: &MutexGuard<ElectionData>) -> bool {
        matches!(
            guard.state,
            ElectionState::Confirmed | ElectionState::ExpiredConfirmed
        )
    }

    pub fn active(&self, hash: &BlockHash) -> bool {
        let guard = self.mutex.lock().unwrap();
        guard.blocks.contains_key(hash)
    }

    pub fn replace_by_weight<'a>(
        &self,
        election: &'a Election,
        mut election_guard: MutexGuard<'a, ElectionData>,
        hash: &BlockHash,
    ) -> (bool, MutexGuard<'a, ElectionData>) {
        let mut replaced_block = BlockHash::zero();
        let winner_hash = election_guard.status.winner.as_ref().unwrap().hash();
        // Sort existing blocks tally
        let mut sorted: Vec<_> = election_guard
            .last_tally
            .iter()
            .map(|(hash, amount)| (*hash, *amount))
            .collect();
        drop(election_guard);

        // Sort in ascending order
        sorted.sort_by(|left, right| right.cmp(left));

        let votes_tally = |votes: &[Arc<Vote>]| {
            let mut result = Amount::zero();
            for vote in votes {
                result += self.ledger.weight(&vote.voting_account);
            }
            result
        };

        // Replace if lowest tally is below inactive cache new block weight
        let inactive_existing = self.vote_cache.lock().unwrap().find(hash);
        let inactive_tally = votes_tally(&inactive_existing);
        if inactive_tally > Amount::zero() && sorted.len() < ELECTION_MAX_BLOCKS {
            // If count of tally items is less than 10, remove any block without tally
            election_guard = election.mutex.lock().unwrap();
            for (hash, _) in &election_guard.last_blocks {
                if sorted.iter().all(|(h, _)| h != hash) && *hash != winner_hash {
                    replaced_block = *hash;
                    break;
                }
            }
        } else if inactive_tally > Amount::zero() && inactive_tally > sorted.first().unwrap().1 {
            if sorted.first().unwrap().0 != winner_hash {
                replaced_block = sorted[0].0;
            } else if inactive_tally > sorted[1].1 {
                // Avoid removing winner
                replaced_block = sorted[1].0;
            }
        }

        let mut replaced = false;
        if !replaced_block.is_zero() {
            self.mutex.lock().unwrap().blocks.remove(&replaced_block);
            election_guard = election.mutex.lock().unwrap();
            self.remove_block(&mut election_guard, &replaced_block);
            replaced = true;
        } else {
            election_guard = election.mutex.lock().unwrap();
        }
        (replaced, election_guard)
    }

    pub fn publish(&self, block: &Arc<BlockEnum>, election: &Election) -> bool {
        let mut election_guard = election.mutex.lock().unwrap();

        // Do not insert new blocks if already confirmed
        let mut result = self.confirmed_locked(&election_guard);
        if !result
            && election_guard.last_blocks.len() >= ELECTION_MAX_BLOCKS
            && !election_guard.last_blocks.contains_key(&block.hash())
        {
            let (replaced, guard) = self.replace_by_weight(election, election_guard, &block.hash());
            election_guard = guard;
            if !replaced {
                result = true;
                self.clear_publish_filter(block);
            }
        }
        if !result {
            if election_guard.last_blocks.get(&block.hash()).is_some() {
                result = true;
                election_guard
                    .last_blocks
                    .insert(block.hash(), Arc::clone(block));
                if election_guard.status.winner.as_ref().unwrap().hash() == block.hash() {
                    election_guard.status.winner = Some(Arc::clone(block));
                    let message = Message::Publish(Publish::new(block.as_ref().clone()));
                    self.tcp_channels.flood_message2(
                        &message,
                        BufferDropPolicy::NoLimiterDrop,
                        1.0,
                    );
                }
            } else {
                election_guard
                    .last_blocks
                    .insert(block.hash(), Arc::clone(block));
            }
        }
        /*
        Result is true if:
        1) election is confirmed or expired
        2) given election contains 10 blocks & new block didn't receive enough votes to replace existing blocks
        3) given block in already in election & election contains less than 10 blocks (replacing block content with new)
        */
        result
    }

    pub fn broadcast_vote(
        &self,
        election: &Election,
        election_guard: &mut MutexGuard<ElectionData>,
    ) {
        if election_guard.last_vote_elapsed() >= self.network.network.vote_broadcast_interval {
            self.broadcast_vote_locked(election_guard, election);
            election_guard.set_last_vote();
        }
    }

    pub fn broadcast_vote_locked(
        &self,
        election_guard: &mut MutexGuard<ElectionData>,
        election: &Election,
    ) {
        let last_vote_elapsed = election_guard.last_vote_elapsed();
        if last_vote_elapsed < self.network.network.vote_broadcast_interval {
            return;
        }
        election_guard.set_last_vote();
        if self.config.enable_voting && self.wallets.voting_reps_count() > 0 {
            self.stats
                .inc(StatType::Election, DetailType::BroadcastVote, Direction::In);

            if self.confirmed_locked(election_guard)
                || self.have_quorum(&self.tally_impl(election_guard))
            {
                self.stats.inc(
                    StatType::Election,
                    DetailType::GenerateVoteFinal,
                    Direction::In,
                );
                let winner = election_guard.status.winner.as_ref().unwrap().hash();
                trace!(qualified_root = ?election.qualified_root, %winner, "type" = "final", "broadcast vote");
                self.final_generator.add(&election.root, &winner); // Broadcasts vote to the network
            } else {
                self.stats.inc(
                    StatType::Election,
                    DetailType::GenerateVoteNormal,
                    Direction::In,
                );
                let winner = election_guard.status.winner.as_ref().unwrap().hash();
                trace!(qualified_root = ?election.qualified_root, %winner, "type" = "normal", "broadcast vote");
                self.generator.add(&election.root, &winner); // Broadcasts vote to the network
            }
        }
    }

    pub fn cleanup_election(
        &self,
        mut guard: MutexGuard<ActiveTransactionsData>,
        election: &Arc<Election>,
    ) {
        // Keep track of election count by election type
        debug_assert!(guard.count_by_behavior(election.behavior) > 0);
        *guard.count_by_behavior_mut(election.behavior) -= 1;

        let election_winner: BlockHash;
        let blocks;
        {
            let election_guard = election.mutex.lock().unwrap();
            blocks = election_guard.last_blocks.clone();
            election_winner = election_guard.status.winner.as_ref().unwrap().hash();
        }

        for hash in blocks.keys() {
            let erased = guard.blocks.remove(hash);
            debug_assert!(erased.is_some());
        }

        guard.roots.erase(&election.qualified_root);

        self.stats.inc(
            self.completion_type(election),
            election.behavior.into(),
            Direction::In,
        );
        trace!(election = ?election, "active stopped");

        drop(guard);

        (self.vacancy_update.lock().unwrap())();

        for (hash, block) in blocks {
            // Notify observers about dropped elections & blocks lost confirmed elections
            if !self.confirmed(election) || hash != election_winner {
                (self.active_stopped_observer)(hash);
            }

            if !self.confirmed(election) {
                // Clear from publish filter
                self.clear_publish_filter(&block);
            }
        }
    }

    fn completion_type(&self, election: &Election) -> StatType {
        if self.confirmed(election) {
            StatType::ActiveConfirmed
        } else if election.failed() {
            StatType::ActiveTimeout
        } else {
            StatType::ActiveDropped
        }
    }

    fn confirmed(&self, election: &Election) -> bool {
        let guard = election.mutex.lock().unwrap();
        self.confirmed_locked(&guard)
    }

    pub fn erase_oldest(&self) {
        let guard = self.mutex.lock().unwrap();
        let mut it = guard.roots.iter_sequenced();
        if let Some((_, election)) = it.next() {
            let election = Arc::clone(election);
            drop(it);
            self.cleanup_election(guard, &election)
        }
    }

    /// Erase elections if we're over capacity
    pub fn trim(&self) {
        /*
         * Both normal and hinted election schedulers are well-behaved, meaning they first check for AEC vacancy before inserting new elections.
         * However, it is possible that AEC will be temporarily overfilled in case it's running at full capacity and election hinting or manual queue kicks in.
         * That case will lead to unwanted churning of elections, so this allows for AEC to be overfilled to 125% until erasing of elections happens.
         */
        while self.vacancy(ElectionBehavior::Normal)
            < -(self.limit(ElectionBehavior::Normal) as i64 / 4)
        {
            self.stats
                .inc(StatType::Active, DetailType::EraseOldest, Direction::In);
            self.erase_oldest();
        }
    }

    fn base_latency(&self) -> Duration {
        if self.network.network.is_dev_network() {
            Duration::from_millis(25)
        } else {
            Duration::from_millis(1000)
        }
    }

    pub fn confirm_req_time(&self, election: &Election) -> Duration {
        match election.behavior {
            ElectionBehavior::Normal | ElectionBehavior::Hinted => self.base_latency() * 5,
            ElectionBehavior::Optimistic => self.base_latency() * 2,
        }
    }

    pub fn broadcast_block_predicate(
        &self,
        election: &Arc<Election>,
        election_guard: &MutexGuard<ElectionData>,
    ) -> bool {
        // Broadcast the block if enough time has passed since the last broadcast (or it's the first broadcast)
        if election.last_block_elapsed() < self.network.network.block_broadcast_interval {
            true
        }
        // Or the current election winner has changed
        else if election_guard.status.winner.as_ref().unwrap().hash()
            != election_guard.last_block_hash
        {
            true
        } else {
            false
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct TallyKey(pub Amount);

impl TallyKey {
    pub fn amount(&self) -> Amount {
        self.0.clone()
    }
}

impl Deref for TallyKey {
    type Target = Amount;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Ord for TallyKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.cmp(&self.0)
    }
}

impl PartialOrd for TallyKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.0.partial_cmp(&self.0)
    }
}

pub struct ActiveTransactionsData {
    pub roots: OrderedRoots,
    pub stopped: bool,
    pub normal_count: usize,
    pub hinted_count: usize,
    pub optimistic_count: usize,
    pub blocks: HashMap<BlockHash, Arc<Election>>,
}

impl ActiveTransactionsData {
    pub fn count_by_behavior(&self, behavior: ElectionBehavior) -> usize {
        match behavior {
            ElectionBehavior::Normal => self.normal_count,
            ElectionBehavior::Hinted => self.hinted_count,
            ElectionBehavior::Optimistic => self.optimistic_count,
        }
    }

    pub fn count_by_behavior_mut(&mut self, behavior: ElectionBehavior) -> &mut usize {
        match behavior {
            ElectionBehavior::Normal => &mut self.normal_count,
            ElectionBehavior::Hinted => &mut self.hinted_count,
            ElectionBehavior::Optimistic => &mut self.optimistic_count,
        }
    }
}

#[derive(Default)]
pub struct OrderedRoots {
    by_root: HashMap<QualifiedRoot, Arc<Election>>,
    sequenced: Vec<QualifiedRoot>,
}

impl OrderedRoots {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, root: QualifiedRoot, election: Arc<Election>) {
        if self.by_root.insert(root.clone(), election).is_none() {
            self.sequenced.push(root);
        }
    }

    pub fn get(&self, root: &QualifiedRoot) -> Option<&Arc<Election>> {
        self.by_root.get(root)
    }

    pub fn erase(&mut self, root: &QualifiedRoot) {
        if let Some(_) = self.by_root.remove(root) {
            self.sequenced.retain(|x| x != root)
        }
    }

    pub fn clear(&mut self) {
        self.sequenced.clear();
        self.by_root.clear();
    }

    pub fn len(&self) -> usize {
        self.sequenced.len()
    }

    pub fn iter_sequenced(&self) -> impl Iterator<Item = (&QualifiedRoot, &Arc<Election>)> {
        self.sequenced
            .iter()
            .map(|r| (r, self.by_root.get(r).unwrap()))
    }
}

pub trait ActiveTransactionsExt {
    /// Confirm this block if quorum is met
    fn confirm_if_quorum(&self, election_lock: MutexGuard<ElectionData>, election: &Arc<Election>);
    fn confirm_once(&self, election_lock: MutexGuard<ElectionData>, election: &Arc<Election>);
    fn process_confirmed(&self, status: ElectionStatus, iteration: u64);
    fn force_confirm(&self, election: &Arc<Election>);
    fn try_confirm(&self, election: &Arc<Election>, hash: &BlockHash);
}

impl ActiveTransactionsExt for Arc<ActiveTransactions> {
    fn force_confirm(&self, election: &Arc<Election>) {
        assert!(self.network.network.is_dev_network());
        let guard = election.mutex.lock().unwrap();
        self.confirm_once(guard, election);
    }

    fn confirm_if_quorum(
        &self,
        mut election_lock: MutexGuard<ElectionData>,
        election: &Arc<Election>,
    ) {
        let tally = self.tally_impl(&mut election_lock);
        let (amount, block) = tally.first_key_value().unwrap();
        let winner_hash = block.hash();
        election_lock.status.tally = amount.amount();
        election_lock.status.final_tally = election_lock.final_weight;
        let status_winner_hash = election_lock.status.winner.as_ref().unwrap().hash();
        let mut sum = Amount::zero();
        for k in tally.keys() {
            sum += k.amount();
        }
        if sum >= self.online_reps.lock().unwrap().delta() && winner_hash != status_winner_hash {
            election_lock.status.winner = Some(Arc::clone(block));
            self.remove_votes(election, &mut election_lock, &status_winner_hash);
            self.block_processor.force(Arc::clone(block));
        }

        if self.have_quorum(&tally) {
            if !election.is_quorum.swap(true, Ordering::SeqCst)
                && self.config.enable_voting
                && self.wallets.voting_reps_count() > 0
            {
                self.final_generator
                    .add(&election.root, &status_winner_hash);
            }
            if election_lock.final_weight >= self.online_reps.lock().unwrap().delta() {
                self.confirm_once(election_lock, election);
            }
        }
    }

    fn confirm_once(&self, mut election_lock: MutexGuard<ElectionData>, election: &Arc<Election>) {
        // This must be kept above the setting of election state, as dependent confirmed elections require up to date changes to election_winner_details
        let mut winners_guard = self.election_winner_details.lock().unwrap();
        let mut status = election_lock.status.clone();
        let old_state = election_lock.state;
        let just_confirmed = old_state != ElectionState::Confirmed;
        election_lock.state = ElectionState::Confirmed;
        if just_confirmed && !winners_guard.contains_key(&status.winner.as_ref().unwrap().hash()) {
            winners_guard.insert(status.winner.as_ref().unwrap().hash(), Arc::clone(election));
            drop(winners_guard);

            election_lock.update_status_to_confirmed(election);
            status = election_lock.status.clone();

            self.recently_confirmed.put(
                election.qualified_root.clone(),
                status.winner.as_ref().unwrap().hash(),
            );

            trace!(
                qualified_root = ?election.qualified_root,
                "election confirmed"
            );
            drop(election_lock);

            let self_l = Arc::clone(&self);
            let election = Arc::clone(election);
            self.workers.push_task(Box::new(move || {
                let block = Arc::clone(status.winner.as_ref().unwrap());
                self_l.process_confirmed(status, 0);
                (election.confirmation_action)(block);
            }));
        }
    }

    fn process_confirmed(&self, status: ElectionStatus, mut iteration: u64) {
        let hash = status.winner.as_ref().unwrap().hash();
        let num_iters = (self.config.block_processor_batch_max_time_ms
            / self.network.node.process_confirmed_interval_ms) as u64
            * 4;
        let block = {
            let tx = self.ledger.read_txn();
            self.ledger.get_block(&tx, &hash)
        };
        if let Some(block) = block {
            trace!(block = ?block,"process confirmed");
            self.confirming_set.add(block.hash());
        } else if iteration < num_iters {
            iteration += 1;
            let self_w = Arc::downgrade(self);
            self.workers.add_delayed_task(
                Duration::from_millis(self.network.node.process_confirmed_interval_ms as u64),
                Box::new(move || {
                    if let Some(self_l) = self_w.upgrade() {
                        self_l.process_confirmed(status, iteration);
                    }
                }),
            );
        } else {
            // Do some cleanup due to this block never being processed by confirmation height processor
            self.remove_election_winner_details(&hash);
        }
    }

    fn try_confirm(&self, election: &Arc<Election>, hash: &BlockHash) {
        let guard = election.mutex.lock().unwrap();
        if let Some(winner) = &guard.status.winner {
            if winner.hash() == *hash {
                if !self.confirmed_locked(&guard) {
                    self.confirm_once(guard, election);
                }
            }
        }
    }
}
