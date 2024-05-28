use super::{ActiveTransactions, ActiveTransactionsExt, VoteProcessorQueue};
use crate::{
    stats::{DetailType, StatType, Stats},
    transport::ChannelEnum,
};
use rsnano_core::{utils::TomlWriter, Vote, VoteCode, VoteSource};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Instant,
};
use tracing::{debug, trace};

#[derive(Clone)]
pub struct VoteProcessorConfig {
    pub max_pr_queue: usize,
    pub max_non_pr_queue: usize,
    pub pr_priority: usize,
}

impl Default for VoteProcessorConfig {
    fn default() -> Self {
        Self {
            max_pr_queue: 256,
            max_non_pr_queue: 32,
            pr_priority: 3,
        }
    }
}

impl VoteProcessorConfig {
    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_usize(
            "max_pr_queue",
            self.max_pr_queue,
            "Maximum number of votes to queue from principal representatives. \ntype:uint64",
        )?;

        toml.put_usize(
            "max_non_pr_queue",
            self.max_non_pr_queue,
            "Maximum number of votes to queue from non-principal representatives. \ntype:uint64",
        )?;

        toml.put_usize(
            "pr_priority",
            self.pr_priority,
            "Priority for votes from principal representatives. Higher priority gets processed more frequently. Non-principal representatives have a baseline priority of 1. \ntype:uint64",
        )
    }
}

pub struct VoteProcessor {
    thread: Mutex<Option<JoinHandle<()>>>,
    queue: Arc<VoteProcessorQueue>,
    active: Arc<ActiveTransactions>,
    stats: Arc<Stats>,
    vote_processed:
        Mutex<Vec<Box<dyn Fn(&Arc<Vote>, &Option<Arc<ChannelEnum>>, VoteCode) + Send + Sync>>>,
    pub total_processed: AtomicU64,
}

impl VoteProcessor {
    const MAX_BATCH_SIZE: usize = 1024 * 4;

    pub fn new(
        queue: Arc<VoteProcessorQueue>,
        active: Arc<ActiveTransactions>,
        stats: Arc<Stats>,
        on_vote: Box<dyn Fn(&Arc<Vote>, &Option<Arc<ChannelEnum>>, VoteCode) + Send + Sync>,
    ) -> Self {
        Self {
            queue,
            active,
            stats,
            vote_processed: Mutex::new(vec![on_vote]),
            thread: Mutex::new(None),
            total_processed: AtomicU64::new(0),
        }
    }

    pub fn stop(&self) {
        self.queue.stop();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap()
        }
    }

    pub fn run(&self) {
        loop {
            self.stats.inc(StatType::VoteProcessor, DetailType::Loop);

            let batch = self.queue.wait_for_votes(Self::MAX_BATCH_SIZE);
            if batch.is_empty() {
                break; //stopped
            }

            let start = Instant::now();

            for (vote, origin) in &batch {
                self.vote_blocking(vote, &origin.channel);
            }

            self.total_processed
                .fetch_add(batch.len() as u64, Ordering::SeqCst);

            let elapsed_millis = start.elapsed().as_millis();
            if batch.len() == Self::MAX_BATCH_SIZE && elapsed_millis > 100 {
                debug!(
                    "Processed {} votes in {} milliseconds (rate of {} votes per second)",
                    batch.len(),
                    elapsed_millis,
                    (batch.len() * 1000) / elapsed_millis as usize
                );
            }
        }
    }

    pub fn vote_blocking(&self, vote: &Arc<Vote>, channel: &Option<Arc<ChannelEnum>>) -> VoteCode {
        let mut result = VoteCode::Invalid;
        if vote.validate().is_ok() {
            let vote_results = self.active.vote(vote, VoteSource::Live);

            // Aggregate results for individual hashes
            let mut replay = false;
            let mut processed = false;
            for (_, hash_result) in vote_results {
                replay |= hash_result == VoteCode::Replay;
                processed |= hash_result == VoteCode::Vote;
            }
            result = if replay {
                VoteCode::Replay
            } else if processed {
                VoteCode::Vote
            } else {
                VoteCode::Indeterminate
            };

            let callbacks = self.vote_processed.lock().unwrap();
            for callback in callbacks.iter() {
                (callback)(vote, channel, result);
            }
        }

        self.stats.inc(StatType::Vote, DetailType::VoteProcessed);
        trace!(?vote, ?result, "vote processed");

        result
    }

    pub fn add_vote_processed_callback(
        &self,
        callback: Box<dyn Fn(&Arc<Vote>, &Option<Arc<ChannelEnum>>, VoteCode) + Send + Sync>,
    ) {
        self.vote_processed.lock().unwrap().push(callback);
    }
}

impl Drop for VoteProcessor {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none());
    }
}

pub trait VoteProcessorExt {
    fn start(&self);
}

impl VoteProcessorExt for Arc<VoteProcessor> {
    fn start(&self) {
        debug_assert!(self.thread.lock().unwrap().is_none());
        let self_l = Arc::clone(self);
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Vote processing".to_string())
                .spawn(Box::new(move || {
                    self_l.run();
                }))
                .unwrap(),
        )
    }
}
