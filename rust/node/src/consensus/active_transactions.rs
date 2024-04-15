use super::{
    Election, ElectionBehavior, ElectionData, ElectionState, ElectionStatus, LocalVoteHistory,
    RecentlyConfirmedCache, VoteGenerator,
};
use crate::{
    block_processing::BlockProcessor, cementation::ConfirmingSet, config::NodeConfig,
    transport::TcpChannels, utils::ThreadPool, wallets::Wallets, NetworkParams, OnlineReps,
};
use num_traits::ops::overflowing::OverflowingSub;
use rsnano_core::{Amount, BlockEnum, BlockHash, QualifiedRoot};
use rsnano_ledger::Ledger;
use std::{
    cmp::max,
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::{atomic::Ordering, Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};
use tracing::trace;

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
    final_generator: Arc<VoteGenerator>,
    tcp_channels: Arc<TcpChannels>,
    pub vacancy_update: Mutex<Box<dyn Fn() + Send + Sync>>,
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
        final_generator: Arc<VoteGenerator>,
        tcp_channels: Arc<TcpChannels>,
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
            final_generator,
            tcp_channels,
            vacancy_update: Mutex::new(Box::new(|| {})),
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
    pub fn vacancy(&self, behavior: ElectionBehavior) -> usize {
        let guard = self.mutex.lock().unwrap();
        match behavior {
            ElectionBehavior::Normal => {
                self.limit(ElectionBehavior::Normal)
                    .overflowing_sub(guard.roots.len())
                    .0
            }
            ElectionBehavior::Hinted | ElectionBehavior::Optimistic => {
                self.limit(behavior)
                    .overflowing_sub(guard.count_by_behavior(behavior))
                    .0
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
