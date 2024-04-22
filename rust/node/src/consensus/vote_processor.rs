use super::{ActiveTransactions, ActiveTransactionsExt, VoteProcessorQueue};
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::ChannelEnum,
};
use rsnano_core::{validate_message, Vote, VoteCode, VoteSource};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Instant,
};
use tracing::{debug, trace};

pub struct VoteProcessor {
    thread: Mutex<Option<JoinHandle<()>>>,
    queue: Arc<VoteProcessorQueue>,
    active: Arc<ActiveTransactions>,
    stats: Arc<Stats>,
    vote_processed: Box<dyn Fn(&Arc<Vote>, &Arc<ChannelEnum>, VoteCode) + Send + Sync>,
    pub total_processed: AtomicU64,
}

impl VoteProcessor {
    pub fn new(
        queue: Arc<VoteProcessorQueue>,
        active: Arc<ActiveTransactions>,
        stats: Arc<Stats>,
        on_vote: Box<dyn Fn(&Arc<Vote>, &Arc<ChannelEnum>, VoteCode) + Send + Sync>,
    ) -> Self {
        Self {
            queue,
            active,
            stats,
            vote_processed: on_vote,
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
        let mut start = Instant::now();
        let mut log_this_iteration;

        loop {
            let votes = self.queue.wait_for_votes();
            if votes.is_empty() {
                break; //stopped
            }
            log_this_iteration = false;
            // TODO: This is a temporary measure to prevent spamming the logs until we can implement a better solution
            if votes.len() > 1024 * 4 {
                /*
                 * Only log the timing information for this iteration if
                 * there are a sufficient number of items for it to be relevant
                 */
                log_this_iteration = true;
                start = Instant::now();
            }
            self.verify_votes(&votes);
            self.total_processed.fetch_add(1, Ordering::SeqCst);

            let elapsed_millis = start.elapsed().as_millis();
            if log_this_iteration && elapsed_millis > 100 {
                debug!(
                    "Processed {} votes in {} milliseconds (rate of {} votes per second)",
                    votes.len(),
                    elapsed_millis,
                    (votes.len() * 1000) / elapsed_millis as usize
                );
            }
        }
    }

    fn verify_votes(&self, votes: &VecDeque<(Arc<Vote>, Arc<ChannelEnum>)>) {
        for (vote, channel) in votes {
            if validate_message(
                &vote.voting_account,
                vote.hash().as_bytes(),
                &vote.signature,
            )
            .is_ok()
            {
                self.vote_blocking(vote, channel, true);
            }
        }
    }

    pub fn vote_blocking(
        &self,
        vote: &Arc<Vote>,
        channel: &Arc<ChannelEnum>,
        validated: bool,
    ) -> VoteCode {
        let mut result = VoteCode::Invalid;
        if validated || vote.validate().is_ok() {
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
            (self.vote_processed)(vote, channel, result);
        }

        self.stats
            .inc(StatType::Vote, DetailType::VoteProcessed, Direction::In);
        trace!(?vote, ?result, "vote processed");

        result
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
