use super::{
    confirmation_solicitor::ConfirmationSolicitor, Election, ElectionBehavior, ElectionData,
    ElectionState, ElectionStatus, ElectionStatusType, LocalVoteHistory, RecentlyConfirmedCache,
    VoteCache, VoteGenerator, VoteInfo, VoteProcessedCallback, NEXT_ELECTION_ID,
};
use crate::{
    block_processing::BlockProcessor,
    cementation::ConfirmingSet,
    config::{NodeConfig, NodeFlags},
    representatives::RepresentativeRegister,
    stats::{DetailType, Sample, StatType, Stats},
    transport::{BufferDropPolicy, Network},
    utils::{HardenedConstants, ThreadPool},
    wallets::Wallets,
    NetworkParams, OnlineReps,
};
use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent, MemoryStream, TomlWriter},
    Account, Amount, BlockEnum, BlockHash, BlockType, QualifiedRoot, Vote, VoteCode, VoteSource,
    VoteWithWeightInfo,
};
use rsnano_ledger::{BlockStatus, Ledger};
use rsnano_messages::{Message, Publish};
use rsnano_store_lmdb::LmdbReadTransaction;
use std::{
    cmp::max,
    collections::{BTreeMap, HashMap},
    mem::size_of,
    ops::Deref,
    sync::{atomic::Ordering, Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime},
};
use tracing::{debug, trace};

const ELECTION_MAX_BLOCKS: usize = 10;

pub type ElectionEndCallback = Box<
    dyn Fn(&ElectionStatus, &Vec<VoteWithWeightInfo>, Account, Amount, bool, bool) + Send + Sync,
>;

pub type AccountBalanceChangedCallback = Box<dyn Fn(&Account, bool) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct ActiveElectionsConfig {
    // Maximum number of simultaneous active elections (AEC size)
    pub size: usize,
    // Limit of hinted elections as percentage of `active_elections_size`
    pub hinted_limit_percentage: usize,
    // Limit of optimistic elections as percentage of `active_elections_size`
    pub optimistic_limit_percentage: usize,
    // Maximum confirmation history size
    pub confirmation_history_size: usize,
    // Maximum cache size for recently_confirmed
    pub confirmation_cache: usize,
}

impl ActiveElectionsConfig {
    pub(crate) fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_usize ("size", self.size, "Number of active elections. Elections beyond this limit have limited survival time.\nWarning: modifying this value may result in a lower confirmation rate. \ntype:uint64,[250..]")?;

        toml.put_usize(
            "hinted_limit_percentage",
            self.hinted_limit_percentage,
            "Limit of hinted elections as percentage of `active_elections_size` \ntype:uint64",
        )?;

        toml.put_usize(
            "optimistic_limit_percentage",
            self.optimistic_limit_percentage,
            "Limit of optimistic elections as percentage of `active_elections_size`. \ntype:uint64",
        )?;

        toml.put_usize ("confirmation_history_size", self.confirmation_history_size, "Maximum confirmation history size. If tracking the rate of block confirmations, the websocket feature is recommended instead. \ntype:uint64")?;

        toml.put_usize ("confirmation_cache", self.confirmation_cache, "Maximum number of confirmed elections kept in cache to prevent restarting an election. \ntype:uint64")
    }
}

impl Default for ActiveElectionsConfig {
    fn default() -> Self {
        Self {
            size: 5000,
            hinted_limit_percentage: 20,
            optimistic_limit_percentage: 10,
            confirmation_history_size: 2048,
            confirmation_cache: 65536,
        }
    }
}

pub struct ActiveElections {
    pub mutex: Mutex<ActiveTransactionsData>,
    pub condition: Condvar,
    network_params: NetworkParams,
    pub online_reps: Arc<Mutex<OnlineReps>>,
    wallets: Arc<Wallets>,
    pub election_winner_details: Mutex<HashMap<BlockHash, Arc<Election>>>,
    node_config: NodeConfig,
    config: ActiveElectionsConfig,
    ledger: Arc<Ledger>,
    confirming_set: Arc<ConfirmingSet>,
    workers: Arc<dyn ThreadPool>,
    pub recently_confirmed: Arc<RecentlyConfirmedCache>,
    /// Helper container for storing recently cemented elections (a block from election might be confirmed but not yet cemented by confirmation height processor)
    pub recently_cemented: Arc<Mutex<BoundedVecDeque<ElectionStatus>>>,
    history: Arc<LocalVoteHistory>,
    block_processor: Arc<BlockProcessor>,
    generator: Arc<VoteGenerator>,
    final_generator: Arc<VoteGenerator>,
    network: Arc<Network>,
    pub vacancy_update: Mutex<Box<dyn Fn() + Send + Sync>>,
    vote_cache: Arc<Mutex<VoteCache>>,
    stats: Arc<Stats>,
    active_started_observer: Mutex<Vec<Box<dyn Fn(BlockHash) + Send + Sync>>>,
    active_stopped_observer: Mutex<Vec<Box<dyn Fn(BlockHash) + Send + Sync>>>,
    vote_processed_observers: Mutex<Vec<VoteProcessedCallback>>,
    activate_successors: Mutex<Box<dyn Fn(LmdbReadTransaction, &Arc<BlockEnum>) + Send + Sync>>,
    election_end: Mutex<Vec<ElectionEndCallback>>,
    account_balance_changed: AccountBalanceChangedCallback,
    representative_register: Arc<Mutex<RepresentativeRegister>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    flags: NodeFlags,
}

impl ActiveElections {
    pub fn new(
        network_params: NetworkParams,
        online_reps: Arc<Mutex<OnlineReps>>,
        wallets: Arc<Wallets>,
        node_config: NodeConfig,
        ledger: Arc<Ledger>,
        confirming_set: Arc<ConfirmingSet>,
        workers: Arc<dyn ThreadPool>,
        history: Arc<LocalVoteHistory>,
        block_processor: Arc<BlockProcessor>,
        generator: Arc<VoteGenerator>,
        final_generator: Arc<VoteGenerator>,
        network: Arc<Network>,
        vote_cache: Arc<Mutex<VoteCache>>,
        stats: Arc<Stats>,
        election_end: ElectionEndCallback,
        account_balance_changed: AccountBalanceChangedCallback,
        representative_register: Arc<Mutex<RepresentativeRegister>>,
        flags: NodeFlags,
        recently_confirmed: Arc<RecentlyConfirmedCache>,
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
            network_params,
            online_reps,
            wallets,
            election_winner_details: Mutex::new(HashMap::new()),
            ledger,
            confirming_set,
            workers,
            recently_confirmed,
            recently_cemented: Arc::new(Mutex::new(BoundedVecDeque::new(
                node_config.active_elections.confirmation_history_size,
            ))),
            config: node_config.active_elections.clone(),
            node_config,
            history,
            block_processor,
            generator,
            final_generator,
            network,
            vacancy_update: Mutex::new(Box::new(|| {})),
            vote_cache,
            stats,
            active_started_observer: Mutex::new(Vec::new()),
            active_stopped_observer: Mutex::new(Vec::new()),
            vote_processed_observers: Mutex::new(Vec::new()),
            activate_successors: Mutex::new(Box::new(|_tx, _block| {})),
            election_end: Mutex::new(vec![election_end]),
            account_balance_changed,
            representative_register,
            thread: Mutex::new(None),
            flags,
        }
    }

    pub fn len(&self) -> usize {
        self.mutex.lock().unwrap().roots.len()
    }

    pub fn add_election_end_callback(&self, f: ElectionEndCallback) {
        self.election_end.lock().unwrap().push(f);
    }

    pub fn add_active_started_callback(&self, f: Box<dyn Fn(BlockHash) + Send + Sync>) {
        self.active_started_observer.lock().unwrap().push(f);
    }

    pub fn add_active_stopped_callback(&self, f: Box<dyn Fn(BlockHash) + Send + Sync>) {
        self.active_stopped_observer.lock().unwrap().push(f);
    }

    pub fn clear_recently_confirmed(&self) {
        self.recently_confirmed.clear();
    }

    pub fn recently_confirmed_count(&self) -> usize {
        self.recently_confirmed.len()
    }

    pub fn recently_cemented_count(&self) -> usize {
        self.recently_cemented.lock().unwrap().len()
    }

    pub fn was_recently_confirmed(&self, hash: &BlockHash) -> bool {
        self.recently_confirmed.hash_exists(hash)
    }

    pub fn latest_recently_confirmed(&self) -> Option<(QualifiedRoot, BlockHash)> {
        self.recently_confirmed.back()
    }

    pub fn insert_recently_confirmed(&self, block: &BlockEnum) {
        self.recently_confirmed
            .put(block.qualified_root(), block.hash());
    }

    pub fn insert_recently_cemented(&self, status: ElectionStatus) {
        self.recently_cemented
            .lock()
            .unwrap()
            .push_back(status.clone());

        // Trigger callback for confirmed block
        let block = status.winner.as_ref().unwrap();
        let account = block.account();
        let amount = self
            .ledger
            .any()
            .block_amount(&self.ledger.read_txn(), &block.hash());
        let mut is_state_send = false;
        let mut is_state_epoch = false;
        if amount.is_some() {
            if block.block_type() == BlockType::State {
                is_state_send = block.is_send();
                is_state_epoch = block.is_epoch();
            }
        }

        let callbacks = self.election_end.lock().unwrap();
        for callback in callbacks.iter() {
            (callback)(
                &status,
                &Vec::new(),
                account,
                amount.unwrap_or_default(),
                is_state_send,
                is_state_epoch,
            );
        }
    }

    pub fn recently_cemented_list(&self) -> BoundedVecDeque<ElectionStatus> {
        self.recently_cemented.lock().unwrap().clone()
    }

    pub fn add_election_winner_details(&self, hash: BlockHash, election: Arc<Election>) {
        self.election_winner_details
            .lock()
            .unwrap()
            .insert(hash, election);
    }

    pub fn election_winner_details_len(&self) -> usize {
        self.election_winner_details.lock().unwrap().len()
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

    pub fn winner(&self, hash: &BlockHash) -> Option<Arc<BlockEnum>> {
        let guard = self.mutex.lock().unwrap();
        guard
            .blocks
            .get(hash)
            .map(|i| Arc::clone(&i.mutex.lock().unwrap().status.winner.as_ref().unwrap()))
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
        let amount = self
            .ledger
            .any()
            .block_amount(tx, &block.hash())
            .unwrap_or_default();
        let is_state_send = block.block_type() == BlockType::State && block.is_send();
        let is_state_epoch = block.block_type() == BlockType::State && block.is_epoch();

        {
            let ended_callbacks = self.election_end.lock().unwrap();
            for callback in ended_callbacks.iter() {
                (callback)(
                    status,
                    votes,
                    account,
                    amount,
                    is_state_send,
                    is_state_epoch,
                );
            }
        }

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
            let loop_interval = self.network_params.network.aec_loop_interval;
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
        self.network.publish_filter.clear_bytes(buf.as_bytes());
    }

    pub fn remove_votes(
        &self,
        election: &Election,
        guard: &mut MutexGuard<ElectionData>,
        hash: &BlockHash,
    ) {
        if self.node_config.enable_voting && self.wallets.voting_reps_count() > 0 {
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
            ElectionBehavior::Normal => self.config.size,
            ElectionBehavior::Hinted => {
                self.config.hinted_limit_percentage * self.config.size / 100
            }
            ElectionBehavior::Optimistic => {
                self.config.optimistic_limit_percentage * self.config.size / 100
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

    pub fn active_root(&self, root: &QualifiedRoot) -> bool {
        let guard = self.mutex.lock().unwrap();
        guard.roots.get(root).is_some()
    }

    pub fn active_block(&self, hash: &BlockHash) -> bool {
        let guard = self.mutex.lock().unwrap();
        guard.blocks.contains_key(hash)
    }

    pub fn active(&self, block: &BlockEnum) -> bool {
        let guard = self.mutex.lock().unwrap();
        guard.blocks.contains_key(&block.hash())
            && guard.roots.get(&block.qualified_root()).is_some()
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
                    self.network
                        .flood_message2(&message, BufferDropPolicy::NoLimiterDrop, 1.0);
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

    /// Broadcasts vote for the current winner of this election
    /// Checks if sufficient amount of time (`vote_generation_interval`) passed since the last vote generation
    pub fn broadcast_vote(
        &self,
        election: &Election,
        election_guard: &mut MutexGuard<ElectionData>,
    ) {
        if election_guard.last_vote_elapsed() >= self.network_params.network.vote_broadcast_interval
        {
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
        if last_vote_elapsed < self.network_params.network.vote_broadcast_interval {
            return;
        }
        election_guard.set_last_vote();
        if self.node_config.enable_voting && self.wallets.voting_reps_count() > 0 {
            self.stats
                .inc(StatType::Election, DetailType::BroadcastVote);

            if self.confirmed_locked(election_guard)
                || self.have_quorum(&self.tally_impl(election_guard))
            {
                self.stats
                    .inc(StatType::Election, DetailType::GenerateVoteFinal);
                let winner = election_guard.status.winner.as_ref().unwrap().hash();
                trace!(qualified_root = ?election.qualified_root, %winner, "type" = "final", "broadcast vote");
                self.final_generator.add(&election.root, &winner); // Broadcasts vote to the network
            } else {
                self.stats
                    .inc(StatType::Election, DetailType::GenerateVoteNormal);
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
        let election_state;
        let blocks;
        {
            let election_guard = election.mutex.lock().unwrap();
            blocks = election_guard.last_blocks.clone();
            election_winner = election_guard.status.winner.as_ref().unwrap().hash();
            election_state = election_guard.state;
        }

        for hash in blocks.keys() {
            let erased = guard.blocks.remove(hash);
            debug_assert!(erased.is_some());
        }

        guard.roots.erase(&election.qualified_root);

        self.stats
            .inc(self.completion_type(election), election.behavior.into());
        trace!(election = ?election, "active stopped");

        debug!(
            "Erased election for blocks: {} (behavior: {:?}, state: {:?})",
            blocks
                .keys()
                .map(|k| k.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            election.behavior,
            election_state
        );

        drop(guard);

        self.stats.sample(
            Sample::ActiveElectionDuration,
            (0, 1000 * 60 * 10),
            election.duration().as_millis() as i64,
        ); // 0-10 minutes range

        (self.vacancy_update.lock().unwrap())();

        for (hash, block) in blocks {
            // Notify observers about dropped elections & blocks lost confirmed elections
            if !self.confirmed(election) || hash != election_winner {
                let callbacks = self.active_stopped_observer.lock().unwrap();
                for callback in callbacks.iter() {
                    (callback)(hash);
                }
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

    pub fn confirmed(&self, election: &Election) -> bool {
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
            self.stats.inc(StatType::Active, DetailType::EraseOldest);
            self.erase_oldest();
        }
    }

    /// Minimum time between broadcasts of the current winner of an election, as a backup to requesting confirmations
    fn base_latency(&self) -> Duration {
        if self.network_params.network.is_dev_network() {
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
        if election.last_block_elapsed() < self.network_params.network.block_broadcast_interval {
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
            self.stats.inc(StatType::Active, DetailType::Loop);
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

        let mut solicitor = ConfirmationSolicitor::new(&self.network_params, &self.network);
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
         * Elections extending the soft config.size limit are flushed after a certain time-to-live cutoff
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

    // Returns a list of elections sorted by difficulty
    pub fn list_active(&self, max: usize) -> Vec<Arc<Election>> {
        self.mutex
            .lock()
            .unwrap()
            .roots
            .iter_sequenced()
            .map(|(_, election)| Arc::clone(election))
            .take(max)
            .collect()
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

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "roots".to_string(),
                    count: guard.roots.len(),
                    sizeof_element: OrderedRoots::ELEMENT_SIZE,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "blocks".to_string(),
                    count: guard.blocks.len(),
                    sizeof_element: size_of::<BlockHash>() + size_of::<Arc<Election>>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "election_winner_details".to_string(),
                    count: self.election_winner_details.lock().unwrap().len(),
                    sizeof_element: size_of::<BlockHash>() + size_of::<Arc<Election>>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "normal".to_string(),
                    count: guard.count_by_behavior(ElectionBehavior::Normal),
                    sizeof_element: 0,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "hinted".to_string(),
                    count: guard.count_by_behavior(ElectionBehavior::Hinted),
                    sizeof_element: 0,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "optimistic".to_string(),
                    count: guard.count_by_behavior(ElectionBehavior::Optimistic),
                    sizeof_element: 0,
                }),
                self.recently_confirmed
                    .collect_container_info("recently_confirmed"),
                ContainerInfoComponent::Composite(
                    "recently_cemented".to_string(),
                    vec![ContainerInfoComponent::Leaf(ContainerInfo {
                        name: "cemented".to_string(),
                        count: self.recently_cemented.lock().unwrap().len(),
                        sizeof_element: size_of::<ElectionStatus>(),
                    })],
                ),
            ],
        )
    }
}

impl Drop for ActiveElections {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none());
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
    pub const ELEMENT_SIZE: usize = size_of::<QualifiedRoot>() * 2 + size_of::<Arc<Election>>();

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

pub trait ActiveElectionsExt {
    fn initialize(&self);
    fn start(&self);
    fn stop(&self);
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

impl ActiveElectionsExt for Arc<ActiveElections> {
    fn initialize(&self) {
        let self_w = Arc::downgrade(self);
        // Register a callback which will get called after a block is cemented
        self.confirming_set
            .add_cemented_observer(Box::new(move |block| {
                if let Some(active) = self_w.upgrade() {
                    active.block_cemented_callback(block);
                }
            }));

        let self_w = Arc::downgrade(self);
        // Register a callback which will get called if a block is already cemented
        self.confirming_set
            .add_already_cemented_observer(Box::new(move |hash| {
                if let Some(active) = self_w.upgrade() {
                    // Depending on timing there is a situation where the election_winner_details is not reset.
                    // This can happen when a block wins an election, and the block is confirmed + observer
                    // called before the block hash gets added to election_winner_details. If the block is confirmed
                    // callbacks have already been done, so we can safely just remove it.
                    active.remove_election_winner_details(&hash);
                }
            }));

        let self_w = Arc::downgrade(self);
        // Notify elections about alternative (forked) blocks
        self.block_processor
            .add_block_processed_observer(Box::new(move |status, context| {
                if matches!(status, BlockStatus::Fork) {
                    if let Some(active) = self_w.upgrade() {
                        active.publish_block(&context.block);
                    }
                }
            }));
    }

    fn start(&self) {
        if self.flags.disable_request_loop {
            return;
        }

        let mut guard = self.thread.lock().unwrap();
        let self_l = Arc::clone(self);
        assert!(guard.is_none());
        *guard = Some(
            std::thread::Builder::new()
                .name("Request loop".to_string())
                .spawn(Box::new(move || {
                    self_l.request_loop();
                }))
                .unwrap(),
        );
    }

    fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        if let Some(join_handle) = self.thread.lock().unwrap().take() {
            join_handle.join().unwrap();
        }
        self.clear();
    }

    fn force_confirm(&self, election: &Arc<Election>) {
        assert!(self.network_params.network.is_dev_network());
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
                && self.node_config.enable_voting
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
        let num_iters = (self.node_config.block_processor_batch_max_time_ms
            / self.network_params.node.process_confirmed_interval_ms)
            as u64
            * 4;
        let block = {
            let tx = self.ledger.read_txn();
            self.ledger.any().get_block(&tx, &hash)
        };
        if let Some(block) = block {
            trace!(block = ?block,"process confirmed");
            self.confirming_set.add(block.hash());
        } else if iteration < num_iters {
            iteration += 1;
            let self_w = Arc::downgrade(self);
            self.workers.add_delayed_task(
                Duration::from_millis(
                    self.network_params.node.process_confirmed_interval_ms as u64,
                ),
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
        debug_assert!(vote.validate().is_ok());

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
        if !self.network_params.network.is_dev_network()
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
        if cemented_bootstrap_count_reached && was_active && !self.flags.disable_activate_successors
        {
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

                self.stats
                    .inc(StatType::Active, DetailType::ElectionBlockConflict);
            }
        }

        result
    }

    fn insert(
        &self,
        block: &Arc<BlockEnum>,
        election_behavior: ElectionBehavior,
    ) -> (bool, Option<Arc<Election>>) {
        let mut election_result = None;
        let mut inserted = false;

        let mut guard = self.mutex.lock().unwrap();

        if guard.stopped {
            return (false, None);
        }

        let root = block.qualified_root();
        let hash = block.hash();
        let existing = guard.roots.get(&root);

        if let Some(existing) = existing {
            election_result = Some(Arc::clone(existing));
        } else {
            if !self.recently_confirmed.root_exists(&root) {
                inserted = true;
                let online_reps = Arc::clone(&self.online_reps);
                let observer_rep_cb = Box::new(move |rep| {
                    // Representative is defined as online if replying to live votes or rep_crawler queries
                    online_reps.lock().unwrap().observe(rep);
                });

                let id = NEXT_ELECTION_ID.fetch_add(1, Ordering::Relaxed);
                let election = Arc::new(Election::new(
                    id,
                    Arc::clone(block),
                    election_behavior,
                    Box::new(|_| {}),
                    observer_rep_cb,
                ));
                guard.roots.insert(root, Arc::clone(&election));
                guard.blocks.insert(hash, Arc::clone(&election));

                // Keep track of election count by election type
                *guard.count_by_behavior_mut(election.behavior) += 1;

                self.stats
                    .inc(StatType::ActiveStarted, election_behavior.into());
                trace!(behavior = ?election_behavior, ?election, "active started");

                debug!(
                    "Started new election for block: {} (behavior: {:?})",
                    hash, election_behavior
                );

                election_result = Some(election);
            } else {
                // result is not set
            }
        }
        drop(guard);

        if inserted {
            debug_assert!(election_result.is_some());

            self.trigger_vote_cache(&hash);

            {
                let callbacks = self.active_started_observer.lock().unwrap();
                for callback in callbacks.iter() {
                    (callback)(hash);
                }
            }
            self.vacancy_update.lock().unwrap()();
        }

        // Votes are generated for inserted or ongoing elections
        if let Some(election) = &election_result {
            let mut guard = election.mutex.lock().unwrap();
            self.broadcast_vote(election, &mut guard);
        }

        self.trim();

        (inserted, election_result)
    }
}
