use super::{
    confirmation_solicitor::ConfirmationSolicitor, Election, ElectionBehavior, ElectionData,
    ElectionState, ElectionStatus, ElectionStatusType, LocalVoteHistory, RecentlyConfirmedCache,
    VoteCache, VoteGenerator, VoteInfo,
};
use crate::{
    block_processing::BlockProcessor,
    cementation::ConfirmingSet,
    config::NodeConfig,
    representatives::RepresentativeRegister,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, TcpChannels},
    utils::{HardenedConstants, ThreadPool},
    wallets::Wallets,
    NetworkParams, OnlineReps,
};
use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{
    utils::MemoryStream, Account, Amount, BlockEnum, BlockHash, BlockType, QualifiedRoot, Vote,
    VoteCode, VoteSource, VoteWithWeightInfo,
};
use rsnano_ledger::Ledger;
use rsnano_messages::{Message, Publish};
use rsnano_store_lmdb::LmdbReadTransaction;
use std::{
    cmp::max,
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::{atomic::Ordering, Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime},
};
use tracing::trace;

const ELECTION_MAX_BLOCKS: usize = 10;

pub type VoteProcessedCallback =
    Box<dyn Fn(&Arc<Vote>, VoteSource, &HashMap<BlockHash, VoteCode>) + Send + Sync>;

pub type ElectionEndCallback = Box<
    dyn Fn(&ElectionStatus, &Vec<VoteWithWeightInfo>, Account, Amount, bool, bool) + Send + Sync,
>;

pub type AccountBalanceChangedCallback = Box<dyn Fn(&Account, bool) + Send + Sync>;

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
    pub recently_cemented: Arc<Mutex<BoundedVecDeque<ElectionStatus>>>,
    history: Arc<LocalVoteHistory>,
    block_processor: Arc<BlockProcessor>,
    generator: Arc<VoteGenerator>,
    final_generator: Arc<VoteGenerator>,
    tcp_channels: Arc<TcpChannels>,
    pub vacancy_update: Mutex<Box<dyn Fn() + Send + Sync>>,
    vote_cache: Arc<Mutex<VoteCache>>,
    stats: Arc<Stats>,
    active_stopped_observer: Box<dyn Fn(BlockHash) + Send + Sync>,
    vote_processed_observers: Mutex<Vec<VoteProcessedCallback>>,
    activate_successors: Mutex<Box<dyn Fn(LmdbReadTransaction, &Arc<BlockEnum>) + Send + Sync>>,
    election_end: ElectionEndCallback,
    account_balance_changed: AccountBalanceChangedCallback,
    representative_register: Arc<Mutex<RepresentativeRegister>>,
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
        election_end: ElectionEndCallback,
        account_balance_changed: AccountBalanceChangedCallback,
        representative_register: Arc<Mutex<RepresentativeRegister>>,
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
            ledger,
            confirming_set,
            workers,
            recently_confirmed: Arc::new(RecentlyConfirmedCache::new(65536)),
            recently_cemented: Arc::new(Mutex::new(BoundedVecDeque::new(
                config.confirmation_history_size,
            ))),
            config,
            history,
            block_processor,
            generator,
            final_generator,
            tcp_channels,
            vacancy_update: Mutex::new(Box::new(|| {})),
            vote_cache,
            stats,
            active_stopped_observer,
            vote_processed_observers: Mutex::new(Vec::new()),
            activate_successors: Mutex::new(Box::new(|_tx, _block| {})),
            election_end,
            account_balance_changed,
            representative_register,
        }
    }

    /*
     * Callbacks
     */
    pub fn add_vote_processed_observer(&self, observer: VoteProcessedCallback) {
        self.vote_processed_observers.lock().unwrap().push(observer);
    }

    pub fn set_activate_successors_callback(
        &self,
        callback: Box<dyn Fn(LmdbReadTransaction, &Arc<BlockEnum>) + Send + Sync>,
    ) {
        *self.activate_successors.lock().unwrap() = callback;
    }

    //--------------------------------------------------------------------------------

    pub fn notify_observers(
        &self,
        tx: &LmdbReadTransaction,
        status: &ElectionStatus,
        votes: &Vec<VoteWithWeightInfo>,
    ) {
        let block = status.winner.as_ref().unwrap();
        let account = block.account();
        let amount = self.ledger.amount(tx, &block.hash()).unwrap_or_default();
        let is_state_send = block.block_type() == BlockType::State && block.is_send();
        let is_state_epoch = block.block_type() == BlockType::State && block.is_epoch();
        (self.election_end)(
            status,
            votes,
            account,
            amount,
            is_state_send,
            is_state_epoch,
        );

        if amount > Amount::zero() {
            (self.account_balance_changed)(&account, false);
            if block.is_send() {
                (self.account_balance_changed)(&block.destination().unwrap(), true);
            }
        }
    }

    pub fn request_loop2<'a>(
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

    pub fn broadcast_block(
        &self,
        solicitor: &mut ConfirmationSolicitor,
        election: &Election,
        election_guard: &mut MutexGuard<ElectionData>,
    ) {
        if self.broadcast_block_predicate(election, election_guard) {
            if solicitor.broadcast(election_guard).is_ok() {
                let last_block_hash = election_guard.last_block_hash;
                self.stats.inc(
                    StatType::Election,
                    if last_block_hash.is_zero() {
                        DetailType::BroadcastBlockInitial
                    } else {
                        DetailType::BroadcastBlockRepeat
                    },
                    Direction::In,
                );
                election.set_last_block();
                election_guard.last_block_hash =
                    election_guard.status.winner.as_ref().unwrap().hash();
            }
        }
    }

    /// Broadcast vote for current election winner. Generates final vote if reached quorum or already confirmed
    /// Requires mutex lock
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

    /// Erase all blocks from active and, if not confirmed, clear digests from network filters
    pub fn cleanup_election<'a>(
        &self,
        mut guard: MutexGuard<'a, ActiveTransactionsData>,
        election: &'a Arc<Election>,
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

    /// Minimum time between broadcasts of the current winner of an election, as a backup to requesting confirmations
    fn base_latency(&self) -> Duration {
        if self.network.network.is_dev_network() {
            Duration::from_millis(25)
        } else {
            Duration::from_millis(1000)
        }
    }

    /// Calculates time delay between broadcasting confirmation requests
    pub fn confirm_req_time(&self, election: &Election) -> Duration {
        match election.behavior {
            ElectionBehavior::Normal | ElectionBehavior::Hinted => self.base_latency() * 5,
            ElectionBehavior::Optimistic => self.base_latency() * 2,
        }
    }

    pub fn broadcast_block_predicate(
        &self,
        election: &Election,
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

    pub fn election(&self, hash: &QualifiedRoot) -> Option<Arc<Election>> {
        let guard = self.mutex.lock().unwrap();
        guard.roots.get(hash).cloned()
    }

    pub fn votes_with_weight(&self, election: &Election) -> Vec<VoteWithWeightInfo> {
        let mut sorted_votes: BTreeMap<TallyKey, Vec<VoteWithWeightInfo>> = BTreeMap::new();
        let guard = election.mutex.lock().unwrap();
        for (&representative, info) in &guard.last_votes {
            if representative == HardenedConstants::get().not_an_account {
                continue;
            }
            let weight = self.ledger.weight(&representative);
            let vote_with_weight = VoteWithWeightInfo {
                representative,
                time: info.time,
                timestamp: info.timestamp,
                hash: info.hash,
                weight,
            };
            sorted_votes
                .entry(TallyKey(weight))
                .or_default()
                .push(vote_with_weight);
        }
        let result: Vec<_> = sorted_votes
            .values_mut()
            .map(|i| std::mem::take(i))
            .flatten()
            .collect();
        result
    }

    pub fn request_loop(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            let stamp = Instant::now();
            self.stats
                .inc(StatType::Active, DetailType::Loop, Direction::In);
            guard = self.request_confirm(guard);
            guard = self.request_loop2(stamp, guard);
        }
    }

    pub fn request_confirm<'a>(
        &'a self,
        guard: MutexGuard<'a, ActiveTransactionsData>,
    ) -> MutexGuard<'a, ActiveTransactionsData> {
        let this_loop_target = guard.roots.len();
        let elections = Self::list_active_impl(this_loop_target, &guard);
        drop(guard);

        let mut solicitor = ConfirmationSolicitor::new(&self.network, &self.tcp_channels);
        solicitor.prepare(
            &self
                .representative_register
                .lock()
                .unwrap()
                .principal_representatives(),
        );

        /*
         * Loop through active elections in descending order of proof-of-work difficulty, requesting confirmation
         *
         * Only up to a certain amount of elections are queued for confirmation request and block rebroadcasting. The remaining elections can still be confirmed if votes arrive
         * Elections extending the soft config.active_elections_size limit are flushed after a certain time-to-live cutoff
         * Flushed elections are later re-activated via frontier confirmation
         */
        for election in elections {
            let confirmed = self.confirmed(&election);
            if confirmed || self.transition_time(&mut solicitor, &election) {
                self.erase(&election.qualified_root);
            }
        }

        solicitor.flush();
        self.mutex.lock().unwrap()
    }

    /// Returns a list of elections sorted by difficulty, mutex must be locked
    fn list_active_impl(
        max: usize,
        guard: &MutexGuard<ActiveTransactionsData>,
    ) -> Vec<Arc<Election>> {
        guard
            .roots
            .iter_sequenced()
            .map(|(_, election)| Arc::clone(election))
            .take(max)
            .collect()
    }

    pub fn erase(&self, root: &QualifiedRoot) -> bool {
        let guard = self.mutex.lock().unwrap();
        if let Some(election) = guard.roots.get(root) {
            let election = Arc::clone(election);
            self.cleanup_election(guard, &election);
            true
        } else {
            false
        }
    }

    fn transition_time(
        &self,
        solicitor: &mut ConfirmationSolicitor,
        election: &Arc<Election>,
    ) -> bool {
        let mut guard = election.mutex.lock().unwrap();
        let mut result = false;
        match guard.state {
            ElectionState::Passive => {
                if self.base_latency() * Election::PASSIVE_DURATION_FACTOR
                    < election.election_start.elapsed()
                {
                    guard
                        .state_change(ElectionState::Passive, ElectionState::Active)
                        .unwrap();
                }
            }
            ElectionState::Active => {
                self.broadcast_vote(election, &mut guard);
                self.broadcast_block(solicitor, election, &mut guard);
                self.send_confirm_req(solicitor, election, &guard);
            }
            ElectionState::Confirmed => {
                result = true; // Return true to indicate this election should be cleaned up
                self.broadcast_block(solicitor, election, &mut guard); // Ensure election winner is broadcasted
                guard
                    .state_change(ElectionState::Confirmed, ElectionState::ExpiredConfirmed)
                    .unwrap();
            }
            ElectionState::ExpiredConfirmed | ElectionState::ExpiredUnconfirmed => {
                unreachable!()
            }
        }

        if !self.confirmed_locked(&guard)
            && election.time_to_live() < election.election_start.elapsed()
        {
            // It is possible the election confirmed while acquiring the mutex
            // state_change returning true would indicate it
            let state = guard.state;
            if guard
                .state_change(state, ElectionState::ExpiredUnconfirmed)
                .is_ok()
            {
                trace!(qualified_root = ?election.qualified_root, "election expired");
                result = true; // Return true to indicate this election should be cleaned up
                guard.status.election_status_type = ElectionStatusType::Stopped;
            }
        }

        result
    }

    fn send_confirm_req(
        &self,
        solicitor: &mut ConfirmationSolicitor,
        election: &Election,
        election_guard: &MutexGuard<ElectionData>,
    ) {
        if self.confirm_req_time(election) < election.last_req_elapsed() {
            if !solicitor.add(election, election_guard) {
                election.set_last_req();
                election
                    .confirmation_request_count
                    .fetch_add(1, Ordering::SeqCst);
            }
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
    /// Distinguishes replay votes, cannot be determined if the block is not in any election
    fn vote(&self, vote: &Arc<Vote>, source: VoteSource) -> HashMap<BlockHash, VoteCode>;
    fn vote2(
        &self,
        election: &Arc<Election>,
        rep: &Account,
        timestamp: u64,
        block_hash: &BlockHash,
        vote_source: VoteSource,
    ) -> VoteCode;
    fn block_cemented_callback(&self, block: &Arc<BlockEnum>);
    fn trigger_vote_cache(&self, hash: &BlockHash) -> bool;
    fn publish_block(&self, block: &Arc<BlockEnum>) -> bool;
    fn insert(
        &self,
        block: &Arc<BlockEnum>,
        election_behavior: ElectionBehavior,
    ) -> (bool, Option<Arc<Election>>);
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

    /// Validate a vote and apply it to the current election if one exists
    /// Distinguishes replay votes, cannot be determined if the block is not in any election
    fn vote(&self, vote: &Arc<Vote>, source: VoteSource) -> HashMap<BlockHash, VoteCode> {
        let mut results = HashMap::new();
        let mut process = HashMap::new();
        let mut inactive = Vec::new(); // Hashes that should be added to inactive vote cache

        {
            let guard = self.mutex.lock().unwrap();
            for hash in &vote.hashes {
                // Ignore duplicate hashes (should not happen with a well-behaved voting node)
                if results.contains_key(hash) {
                    continue;
                }

                if let Some(existing) = guard.blocks.get(hash) {
                    process.insert(*hash, Arc::clone(existing));
                } else if !self.recently_confirmed.hash_exists(hash) {
                    inactive.push(*hash);
                    results.insert(*hash, VoteCode::Indeterminate);
                } else {
                    results.insert(*hash, VoteCode::Replay);
                }
            }
        }

        for (block_hash, election) in process {
            let vote_result = self.vote2(
                &election,
                &vote.voting_account,
                vote.timestamp(),
                &block_hash,
                source,
            );
            results.insert(block_hash, vote_result);
        }

        let observers = self.vote_processed_observers.lock().unwrap();
        for o in observers.iter() {
            o(vote, source, &results);
        }

        results
    }

    fn vote2(
        &self,
        election: &Arc<Election>,
        rep: &Account,
        timestamp: u64,
        block_hash: &BlockHash,
        vote_source: VoteSource,
    ) -> VoteCode {
        let weight = self.ledger.weight(rep);
        if !self.network.network.is_dev_network()
            && weight <= self.online_reps.lock().unwrap().minimum_principal_weight()
        {
            return VoteCode::Indeterminate;
        }

        let mut guard = election.mutex.lock().unwrap();

        if let Some(last_vote) = guard.last_votes.get(rep) {
            if last_vote.timestamp > timestamp {
                return VoteCode::Replay;
            }
            if last_vote.timestamp == timestamp && !(last_vote.hash < *block_hash) {
                return VoteCode::Replay;
            }

            let max_vote = timestamp == u64::MAX && last_vote.timestamp < timestamp;

            let mut past_cooldown = true;
            // Only cooldown live votes
            if vote_source == VoteSource::Live {
                let cooldown = self.cooldown_time(weight);
                past_cooldown = last_vote.time <= SystemTime::now() - cooldown;
            }

            if !max_vote && !past_cooldown {
                return VoteCode::Ignored;
            }
        }
        guard
            .last_votes
            .insert(*rep, VoteInfo::new(timestamp, *block_hash));

        if vote_source == VoteSource::Live {
            (election.live_vote_action)(*rep);
        }

        self.stats.inc(
            StatType::Election,
            if vote_source == VoteSource::Live {
                DetailType::VoteNew
            } else {
                DetailType::VoteCached
            },
            Direction::In,
        );
        trace!(
            qualified_root = ?election.qualified_root,
            account = %rep,
            hash = %block_hash,
            timestamp,
            ?vote_source,
            ?weight,
            "vote processed");

        if !self.confirmed_locked(&guard) {
            self.confirm_if_quorum(guard, election);
        }
        VoteCode::Vote
    }

    fn block_cemented_callback(&self, block: &Arc<BlockEnum>) {
        if let Some(election) = self.election(&block.qualified_root()) {
            self.try_confirm(&election, &block.hash());
        }
        let votes: Vec<VoteWithWeightInfo>;
        let mut status: ElectionStatus;
        let election = self.remove_election_winner_details(&block.hash());
        if let Some(election) = &election {
            status = election.mutex.lock().unwrap().status.clone();
            votes = self.votes_with_weight(election);
        } else {
            status = ElectionStatus {
                winner: Some(Arc::clone(block)),
                ..Default::default()
            };
            votes = Vec::new();
        }
        if self.confirming_set.exists(&block.hash()) {
            status.election_status_type = ElectionStatusType::ActiveConfirmedQuorum;
        } else if election.is_some() {
            status.election_status_type = ElectionStatusType::ActiveConfirmationHeight;
        } else {
            status.election_status_type = ElectionStatusType::InactiveConfirmationHeight;
        }

        self.recently_cemented
            .lock()
            .unwrap()
            .push_back(status.clone());
        let tx = self.ledger.read_txn();
        self.notify_observers(&tx, &status, &votes);
        let cemented_bootstrap_count_reached =
            self.ledger.cemented_count() >= self.ledger.bootstrap_weight_max_blocks();
        let was_active = status.election_status_type == ElectionStatusType::ActiveConfirmedQuorum
            || status.election_status_type == ElectionStatusType::ActiveConfirmationHeight;

        // Next-block activations are only done for blocks with previously active elections
        if cemented_bootstrap_count_reached && was_active {
            let guard = self.activate_successors.lock().unwrap();
            (guard)(tx, block);
        }
    }

    fn trigger_vote_cache(&self, hash: &BlockHash) -> bool {
        let cached = self.vote_cache.lock().unwrap().find(hash);
        for cached_vote in &cached {
            self.vote(cached_vote, VoteSource::Cache);
        }
        !cached.is_empty()
    }

    fn publish_block(&self, block: &Arc<BlockEnum>) -> bool {
        let mut guard = self.mutex.lock().unwrap();
        let root = block.qualified_root();
        let mut result = true;
        if let Some(election) = guard.roots.get(&root) {
            let election = Arc::clone(election);
            drop(guard);
            result = self.publish(block, &election);
            if !result {
                guard = self.mutex.lock().unwrap();
                guard.blocks.insert(block.hash(), election);
                drop(guard);

                self.trigger_vote_cache(&block.hash());

                self.stats.inc(
                    StatType::Active,
                    DetailType::ElectionBlockConflict,
                    Direction::In,
                );
            }
        }

        result
    }

    fn insert(
        &self,
        block: &Arc<BlockEnum>,
        election_behavior: ElectionBehavior,
    ) -> (bool, Option<Arc<Election>>) {
        let guard = self.mutex.lock().unwrap();

        //nano::election_insertion_result result;

        if guard.stopped {
            return (false, None);
        }
        todo!()

        //auto const root (block_a->qualified_root ());
        //auto const hash = block_a->hash ();
        //auto const existing_handle = rsnano::rsn_active_transactions_lock_roots_find (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data ());
        //std::shared_ptr<nano::election> existing{};
        //if (existing_handle != nullptr)
        //{
        //    existing = std::make_shared<nano::election> (existing_handle);
        //}

        //if (existing == nullptr)
        //{
        //    if (!recently_confirmed ().exists (root))
        //    {
        //        result.inserted = true;
        //        auto observe_rep_cb = [&node = node] (auto const & rep_a) {
        //            // Representative is defined as online if replying to live votes or rep_crawler queries
        //            node.online_reps.observe (rep_a);
        //        };
        //        auto hash (block_a->hash ());
        //        result.election = nano::make_shared<nano::election> (node, block_a, nullptr, observe_rep_cb, election_behavior_a);
        //        rsnano::rsn_active_transactions_lock_roots_insert (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data (), result.election->handle);
        //        rsnano::rsn_active_transactions_lock_blocks_insert (guard.handle, hash.bytes.data (), result.election->handle);

        //        // Keep track of election count by election type
        //        debug_assert (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (result.election->behavior ())) >= 0);
        //        rsnano::rsn_active_transactions_lock_count_by_behavior_inc (guard.handle, static_cast<uint8_t> (result.election->behavior ()));

        //        node.stats->inc (nano::stat::type::active_started, to_stat_detail (election_behavior_a));
        //        node.logger->trace (nano::log::type::active_transactions, nano::log::detail::active_started,
        //        nano::log::arg{ "behavior", election_behavior_a },
        //        nano::log::arg{ "election", result.election });
        //    }
        //    else
        //    {
        //        // result is not set
        //    }
        //}
        //else
        //{
        //    result.election = existing;
        //}
        //guard.unlock ();

        //if (result.inserted)
        //{
        //    debug_assert (result.election);

        //    trigger_vote_cache (hash);

        //    node.observers->active_started.notify (hash);
        //    vacancy_update ();
        //}

        //// Votes are generated for inserted or ongoing elections
        //if (result.election)
        //{
        //    auto guard{ result.election->lock () };
        //    broadcast_vote (*result.election, guard);
        //}

        //trim ();

        //return result;
    }
}
