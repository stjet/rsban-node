use std::{
    collections::{HashSet, VecDeque},
    mem::size_of,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent, Logger},
    Account,
};
use rsnano_ledger::Ledger;

use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::ChannelEnum,
    OnlineReps,
};

use super::Vote;

pub struct VoteProcessorQueue {
    data: Mutex<VoteProcessorQueueData>,
    condition: Condvar,
    max_votes: usize,
    stats: Arc<Stats>,
    online_reps: Arc<Mutex<OnlineReps>>,
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
}

impl VoteProcessorQueue {
    pub fn new(
        max_votes: usize,
        stats: Arc<Stats>,
        online_reps: Arc<Mutex<OnlineReps>>,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            data: Mutex::new(VoteProcessorQueueData {
                stopped: false,
                votes: VecDeque::new(),
                representatives_1: HashSet::new(),
                representatives_2: HashSet::new(),
                representatives_3: HashSet::new(),
            }),
            condition: Condvar::new(),
            max_votes,
            online_reps,
            stats,
            logger,
            ledger,
        }
    }

    pub fn len(&self) -> usize {
        self.data.lock().unwrap().votes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.lock().unwrap().votes.is_empty()
    }

    pub fn vote(&self, vote_a: &Arc<Vote>, channel_a: &Arc<ChannelEnum>) -> bool {
        let mut process = false;
        let mut guard = self.data.lock().unwrap();
        if !guard.stopped {
            // Level 0 (< 0.1%)
            if (guard.votes.len() as f32) < (6.0 / 9.0 * self.max_votes as f32) {
                process = true;
            }
            // Level 1 (0.1-1%)
            else if (guard.votes.len() as f32) < (7.0 / 9.0 * self.max_votes as f32) {
                process = guard.representatives_1.contains(&vote_a.voting_account);
            }
            // Level 2 (1-5%)
            else if (guard.votes.len() as f32) < (8.0 / 9.0 * self.max_votes as f32) {
                process = guard.representatives_2.contains(&vote_a.voting_account);
            }
            // Level 3 (> 5%)
            else if guard.votes.len() < self.max_votes {
                process = guard.representatives_3.contains(&vote_a.voting_account);
            }
            if process {
                guard
                    .votes
                    .push_back((Arc::clone(vote_a), Arc::clone(channel_a)));
                drop(guard);
                self.condition.notify_all();
            } else {
                self.stats
                    .inc(StatType::Vote, DetailType::VoteOverflow, Direction::In);
            }
        }
        return !process;
    }

    pub fn calculate_weights(&self) {
        let mut guard = self.data.lock().unwrap();
        if !guard.stopped {
            guard.representatives_1.clear();
            guard.representatives_2.clear();
            guard.representatives_3.clear();
            let supply = { self.online_reps.lock().unwrap().trended() };
            let rep_amounts = self.ledger.cache.rep_weights.get_rep_amounts();
            for representative in rep_amounts.keys() {
                let weight = self.ledger.weight(representative);
                if weight > supply / 1000 {
                    // 0.1% or above (level 1)
                    guard.representatives_1.insert(*representative);
                    if weight > supply / 100 {
                        // 1% or above (level 2)
                        guard.representatives_2.insert(*representative);
                        if weight > supply / 20 {
                            // 5% or above (level 3)
                            guard.representatives_3.insert(*representative);
                        }
                    }
                }
            }
        }
    }

    pub fn wait_for_votes(&self) -> VecDeque<(Arc<Vote>, Arc<ChannelEnum>)> {
        let mut guard = self.data.lock().unwrap();
        loop {
            if guard.stopped {
                return VecDeque::new();
            }

            if !guard.votes.is_empty() {
                let mut votes = VecDeque::new();
                std::mem::swap(&mut guard.votes, &mut votes);
                drop(guard);
                self.condition.notify_all();
                return votes;
            } else {
                guard = self.condition.wait(guard).unwrap();
            }
        }
    }

    pub fn flush(&self) {
        let guard = self.data.lock().unwrap();
        let result = self
            .condition
            .wait_timeout_while(guard, Duration::from_secs(60), |l| {
                !l.stopped && !l.votes.is_empty()
            });

        if result.is_err() {
            self.logger
                .always_log("WARNING: vote_processor_queue::flush timeout while waiting for flush")
        }
    }

    pub fn reps_contains(&self, reps_id: u8, account: &Account) -> bool {
        let guard = self.data.lock().unwrap();
        let resp = match reps_id {
            1 => &guard.representatives_1,
            2 => &guard.representatives_2,
            _ => &guard.representatives_3,
        };

        resp.contains(account)
    }

    pub fn clear(&self) {
        {
            let mut guard = self.data.lock().unwrap();
            guard.votes.clear();
        }
        self.condition.notify_all();
    }

    pub fn stop(&self) {
        {
            let mut guard = self.data.lock().unwrap();
            guard.stopped = true;
        }
        self.condition.notify_all();
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.data.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "votes".to_string(),
                    count: guard.votes.len(),
                    sizeof_element: size_of::<(Arc<Vote>, Arc<ChannelEnum>)>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "representatives_1".to_string(),
                    count: guard.representatives_1.len(),
                    sizeof_element: size_of::<Account>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "representatives_2".to_string(),
                    count: guard.representatives_2.len(),
                    sizeof_element: size_of::<Account>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "representatives_3".to_string(),
                    count: guard.representatives_3.len(),
                    sizeof_element: size_of::<Account>(),
                }),
            ],
        )
    }
}

struct VoteProcessorQueueData {
    stopped: bool,
    votes: VecDeque<(Arc<Vote>, Arc<ChannelEnum>)>,

    /// Representatives levels for random early detection
    representatives_1: HashSet<Account>,
    representatives_2: HashSet<Account>,
    representatives_3: HashSet<Account>,
}
