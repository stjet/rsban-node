use super::{Election, RecentlyConfirmedCache, VoteApplier, VoteCache};
use crate::{consensus::VoteApplierExt, stats::Stats, NetworkParams};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockHash, Vote, VoteCode, VoteSource,
};
use std::{
    collections::HashMap,
    mem::size_of,
    sync::{Arc, Condvar, Mutex, Weak},
    thread::JoinHandle,
    time::Duration,
};

pub struct VoteRouter {
    thread: Mutex<Option<JoinHandle<()>>>,
    shared: Arc<(Condvar, Mutex<State>)>,
    vote_processed_observers: Mutex<Vec<VoteProcessedCallback>>,
    vote_cache: Arc<Mutex<VoteCache>>,
    recently_confirmed: Arc<RecentlyConfirmedCache>,
    network_params: NetworkParams,
    stats: Arc<Stats>,
    vote_applier: Arc<VoteApplier>,
}

impl VoteRouter {
    pub fn new(
        vote_cache: Arc<Mutex<VoteCache>>,
        recently_confirmed: Arc<RecentlyConfirmedCache>,
        network_params: NetworkParams,
        stats: Arc<Stats>,
        vote_applier: Arc<VoteApplier>,
    ) -> Self {
        Self {
            thread: Mutex::new(None),
            shared: Arc::new((
                Condvar::new(),
                Mutex::new(State {
                    stopped: false,
                    elections: HashMap::new(),
                }),
            )),
            vote_processed_observers: Mutex::new(Vec::new()),
            vote_cache,
            recently_confirmed,
            network_params,
            stats,
            vote_applier,
        }
    }

    pub fn start(&self) {
        let shared = self.shared.clone();
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Voute router".to_owned())
                .spawn(move || {
                    let (condition, state) = &*shared;
                    let mut guard = state.lock().unwrap();
                    while !guard.stopped {
                        guard.clean_up();
                        guard = condition
                            .wait_timeout_while(guard, Duration::from_secs(15), |g| !g.stopped)
                            .unwrap()
                            .0;
                    }
                })
                .unwrap(),
        )
    }

    pub fn stop(&self) {
        self.shared.1.lock().unwrap().stopped = true;
        self.shared.0.notify_all();
        if let Some(thread) = self.thread.lock().unwrap().take() {
            thread.join().unwrap();
        }
    }

    pub fn add_vote_processed_observer(&self, observer: VoteProcessedCallback) {
        self.vote_processed_observers.lock().unwrap().push(observer);
    }

    pub fn connect(&self, hash: BlockHash, election: Weak<Election>) {
        self.shared
            .1
            .lock()
            .unwrap()
            .elections
            .insert(hash, election);
    }

    pub fn disconnect_election(&self, election: &Election) {
        let mut state = self.shared.1.lock().unwrap();
        let election_guard = election.mutex.lock().unwrap();
        for hash in election_guard.last_blocks.keys() {
            state.elections.remove(hash);
        }
    }

    pub fn disconnect(&self, hash: &BlockHash) {
        let mut state = self.shared.1.lock().unwrap();
        state.elections.remove(hash);
    }

    pub fn election(&self, hash: &BlockHash) -> Option<Arc<Election>> {
        let state = self.shared.1.lock().unwrap();
        state.elections.get(hash)?.upgrade()
    }
    ///
    /// Validate a vote and apply it to the current election if one exists
    pub fn vote_filter(
        &self,
        vote: &Arc<Vote>,
        source: VoteSource,
        filter: &BlockHash,
    ) -> HashMap<BlockHash, VoteCode> {
        debug_assert!(vote.validate().is_ok());

        let mut results = HashMap::new();
        let mut process = HashMap::new();
        let mut inactive = Vec::new(); // Hashes that should be added to inactive vote cache

        {
            let guard = self.shared.1.lock().unwrap();
            for hash in &vote.hashes {
                // Ignore votes for other hashes if a filter is set
                if !filter.is_zero() && hash != filter {
                    continue;
                }

                // Ignore duplicate hashes (should not happen with a well-behaved voting node)
                if results.contains_key(hash) {
                    continue;
                }

                if let Some(existing) = guard.elections.get(hash) {
                    if let Some(election) = existing.upgrade() {
                        process.insert(*hash, election.clone());
                    }
                }

                if process.contains_key(hash) {
                    // There was an active election for hash
                } else if !self.recently_confirmed.hash_exists(hash) {
                    inactive.push(*hash);
                    results.insert(*hash, VoteCode::Indeterminate);
                } else {
                    results.insert(*hash, VoteCode::Replay);
                }
            }
        }

        for (block_hash, election) in process {
            let vote_result = self.vote_applier.vote(
                &election,
                &vote.voting_account,
                vote.timestamp(),
                &block_hash,
                source,
            );
            results.insert(block_hash, vote_result);
        }

        self.on_vote_processed(vote, source, &results);

        results
    }

    /// Validate a vote and apply it to the current election if one exists
    pub fn vote(&self, vote: &Arc<Vote>, source: VoteSource) -> HashMap<BlockHash, VoteCode> {
        self.vote_filter(vote, source, &BlockHash::zero())
    }

    pub fn active(&self, hash: &BlockHash) -> bool {
        let state = self.shared.1.lock().unwrap();
        if let Some(existing) = state.elections.get(hash) {
            existing.strong_count() > 0
        } else {
            false
        }
    }

    fn on_vote_processed(
        &self,
        vote: &Arc<Vote>,
        source: VoteSource,
        results: &HashMap<BlockHash, VoteCode>,
    ) {
        let observers = self.vote_processed_observers.lock().unwrap();
        for o in observers.iter() {
            o(vote, source, results);
        }
    }

    pub fn trigger_vote_cache(&self, hash: &BlockHash) -> bool {
        let cached = self.vote_cache.lock().unwrap().find(hash);
        for cached_vote in &cached {
            self.vote_filter(cached_vote, VoteSource::Cache, hash);
        }
        !cached.is_empty()
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.shared.1.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "elections".to_owned(),
                count: guard.elections.len(),
                sizeof_element: size_of::<BlockHash>() + size_of::<Weak<Election>>(),
            })],
        )
    }
}

impl Drop for VoteRouter {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none())
    }
}

struct State {
    stopped: bool,
    // Mapping of block hashes to elections.
    // Election already contains the associated block
    elections: HashMap<BlockHash, Weak<Election>>,
}

impl State {
    fn clean_up(&mut self) {
        self.elections
            .retain(|_, election| election.strong_count() > 0)
    }
}

pub type VoteProcessedCallback =
    Box<dyn Fn(&Arc<Vote>, VoteSource, &HashMap<BlockHash, VoteCode>) + Send + Sync>;
