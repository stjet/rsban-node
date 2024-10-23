use super::{RepTier, RepTiers, VoteProcessorConfig};
use crate::{
    stats::{DetailType, StatType, Stats},
    transport::FairQueue,
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Vote, VoteSource,
};
use rsnano_network::{ChannelId, DeadChannelCleanupStep};
use std::{
    collections::VecDeque,
    mem::size_of,
    sync::{Arc, Condvar, Mutex},
};
use strum::IntoEnumIterator;


#[derive(Default)]
pub struct VoteProcessorInfo {
    pub queues: Vec<VoteQueueInfo>,
    pub total_size: usize,
}

pub struct VoteQueueInfo {
    pub source: RepTier,
    pub size: usize,
    pub max_size: usize,
}

pub struct VoteProcessorQueue {
    data: Mutex<VoteProcessorQueueData>,
    condition: Condvar,
    pub config: VoteProcessorConfig,
    stats: Arc<Stats>,
    rep_tiers: Arc<RepTiers>,
}

impl VoteProcessorQueue {
    pub fn new(config: VoteProcessorConfig, stats: Arc<Stats>, rep_tiers: Arc<RepTiers>) -> Self {
        let conf = config.clone();
        Self {
            data: Mutex::new(VoteProcessorQueueData {
                stopped: false,
                queue: FairQueue::new(
                    Box::new(move |(tier, _)| match tier {
                        RepTier::Tier1 | RepTier::Tier2 | RepTier::Tier3 => conf.max_pr_queue,
                        RepTier::None => conf.max_non_pr_queue,
                    }),
                    Box::new(move |(tier, _)| match tier {
                        RepTier::Tier3 => conf.pr_priority * conf.pr_priority * conf.pr_priority,
                        RepTier::Tier2 => conf.pr_priority * conf.pr_priority,
                        RepTier::Tier1 => conf.pr_priority,
                        RepTier::None => 1,
                    }),
                ),
            }),
            condition: Condvar::new(),
            config,
            stats,
            rep_tiers,
        }
    }

    pub fn len(&self) -> usize {
        self.data.lock().unwrap().queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.lock().unwrap().queue.is_empty()
    }

    /// Queue vote for processing. @returns true if the vote was queued
    pub fn vote(&self, vote: Arc<Vote>, channel_id: ChannelId, source: VoteSource) -> bool {
        let tier = self.rep_tiers.tier(&vote.voting_account);

        let added = {
            let mut guard = self.data.lock().unwrap();
            guard.queue.push((tier, channel_id), (vote, source))
        };

        if added {
            self.stats.inc(StatType::VoteProcessor, DetailType::Process);
            self.stats.inc(StatType::VoteProcessorTier, tier.into());
            self.condition.notify_one();
        } else {
            self.stats
                .inc(StatType::VoteProcessor, DetailType::Overfill);
            self.stats.inc(StatType::VoteProcessorOverfill, tier.into());
        }

        added
    }

    pub(crate) fn wait_for_votes(
        &self,
        max_batch_size: usize,
    ) -> VecDeque<((RepTier, ChannelId), (Arc<Vote>, VoteSource))> {
        let mut guard = self.data.lock().unwrap();
        loop {
            if guard.stopped {
                return VecDeque::new();
            }

            if !guard.queue.is_empty() {
                return guard.queue.next_batch(max_batch_size);
            } else {
                guard = self.condition.wait(guard).unwrap();
            }
        }
    }

    pub fn clear(&self) {
        {
            let mut guard = self.data.lock().unwrap();
            guard.queue.clear();
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

    pub fn info(&self) -> VoteProcessorInfo {
        let guard = self.data.lock().unwrap();

        let mut queues_info = std::collections::HashMap::new();

        // Iterate over all queues in the FairQueue
        for (key, entry) in guard.queue.iter_queues() {
            let (tier, _channel_id) = *key;
            let size = entry.len();

            queues_info
                .entry(tier)
                .and_modify(|e| *e += size)
                .or_insert(size);
        }

        // Prepare the VoteQueueInfo for each RepTier
        let mut queues = Vec::new();

        for tier in RepTier::iter() {
            let size = queues_info.get(&tier).cloned().unwrap_or(0);
            let max_size = match tier {
                RepTier::Tier1 | RepTier::Tier2 | RepTier::Tier3 => self.config.max_pr_queue,
                RepTier::None => self.config.max_non_pr_queue,
            };

            queues.push(VoteQueueInfo {
                source: tier,
                size,
                max_size,
            });
        }

        VoteProcessorInfo {
            queues,
            total_size: guard.queue.len(),
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.data.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "votes".to_string(),
                    count: guard.queue.len(),
                    sizeof_element: size_of::<(Arc<Vote>, VoteSource)>(),
                }),
                guard.queue.collect_container_info("queue"),
            ],
        )
    }
}

pub struct VoteProcessorQueueCleanup(Arc<VoteProcessorQueue>);

impl VoteProcessorQueueCleanup {
    pub fn new(queue: Arc<VoteProcessorQueue>) -> Self {
        Self(queue)
    }
}

impl DeadChannelCleanupStep for VoteProcessorQueueCleanup {
    fn clean_up_dead_channels(&self, dead_channel_ids: &[ChannelId]) {
        let mut guard = self.0.data.lock().unwrap();
        for channel_id in dead_channel_ids {
            for tier in RepTier::iter() {
                guard.queue.remove(&(tier, *channel_id));
            }
        }
    }
}

struct VoteProcessorQueueData {
    stopped: bool,
    queue: FairQueue<(RepTier, ChannelId), (Arc<Vote>, VoteSource)>,
}
