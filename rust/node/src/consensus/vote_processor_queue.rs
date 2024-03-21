use super::{RepTier, RepTiers};
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::ChannelEnum,
    OnlineReps,
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Vote,
};
use rsnano_ledger::Ledger;
use std::{
    collections::VecDeque,
    mem::size_of,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use tracing::error;

pub struct VoteProcessorQueue {
    data: Mutex<VoteProcessorQueueData>,
    condition: Condvar,
    max_votes: usize,
    stats: Arc<Stats>,
    online_reps: Arc<Mutex<OnlineReps>>,
    ledger: Arc<Ledger>,
    rep_tiers: Arc<RepTiers>,
}

impl VoteProcessorQueue {
    pub fn new(
        max_votes: usize,
        stats: Arc<Stats>,
        online_reps: Arc<Mutex<OnlineReps>>,
        ledger: Arc<Ledger>,
        rep_tiers: Arc<RepTiers>,
    ) -> Self {
        Self {
            data: Mutex::new(VoteProcessorQueueData {
                stopped: false,
                votes: VecDeque::new(),
            }),
            condition: Condvar::new(),
            max_votes,
            online_reps,
            stats,
            ledger,
            rep_tiers,
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
            let tier = self.rep_tiers.tier(&vote_a.voting_account);

            // Level 0 (< 0.1%)
            if (guard.votes.len() as f32) < (6.0 / 9.0 * self.max_votes as f32) {
                process = true;
            }
            // Level 1 (0.1-1%)
            else if (guard.votes.len() as f32) < (7.0 / 9.0 * self.max_votes as f32) {
                process = matches!(tier, RepTier::Tier1);
            }
            // Level 2 (1-5%)
            else if (guard.votes.len() as f32) < (8.0 / 9.0 * self.max_votes as f32) {
                process = matches!(tier, RepTier::Tier2);
            }
            // Level 3 (> 5%)
            else if guard.votes.len() < self.max_votes {
                process = matches!(tier, RepTier::Tier3);
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
            error!("vote_processor_queue::flush timeout while waiting for flush")
        }
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
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "votes".to_string(),
                count: guard.votes.len(),
                sizeof_element: size_of::<(Arc<Vote>, Arc<ChannelEnum>)>(),
            })],
        )
    }
}

struct VoteProcessorQueueData {
    stopped: bool,
    votes: VecDeque<(Arc<Vote>, Arc<ChannelEnum>)>,
}
