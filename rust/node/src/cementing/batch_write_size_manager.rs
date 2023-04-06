use std::{
    cmp::max,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

/// Blocks are cemented in batches. The BatchWriteSizeManager dynamically adjusts
/// that batch size so that writing a batch should not take more than 250 ms.
pub(crate) struct BatchWriteSizeManager {
    pub batch_write_size: Arc<AtomicUsize>,
}

impl BatchWriteSizeManager {
    const MINIMUM_BATCH_WRITE_SIZE: usize = 16384;
    const MAXIMUM_BATCH_WRITE_TIME: Duration = Duration::from_millis(250);

    const MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF: Duration =
        eighty_percent_of(Self::MAXIMUM_BATCH_WRITE_TIME);

    pub fn new() -> Self {
        Self {
            batch_write_size: Arc::new(AtomicUsize::new(Self::MINIMUM_BATCH_WRITE_SIZE)),
        }
    }

    pub fn current_size(&self) -> usize {
        self.batch_write_size.load(Ordering::SeqCst)
    }

    /// Include a tolerance to save having to potentially wait on the block processor if the number of blocks to cement is only a bit higher than the max.
    pub fn current_size_with_tolerance(&self) -> usize {
        let size = self.current_size();
        size + (size / 10)
    }

    pub fn set_size(&self, size: usize) {
        self.batch_write_size.store(size, Ordering::SeqCst);
    }

    pub fn adjust_size(&self, time_spent_cementing: Duration) {
        // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
        if time_spent_cementing > Self::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce();
        } else if time_spent_cementing < Self::MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF {
            // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
            self.increase();
        }
    }

    fn increase(&self) {
        self.batch_write_size
            .fetch_add(self.amount_to_change(), Ordering::SeqCst);
    }

    fn reduce(&self) {
        // Reduce (unless we have hit a floor)
        let new_size = max(
            BatchWriteSizeManager::MINIMUM_BATCH_WRITE_SIZE,
            self.current_size() - self.amount_to_change(),
        );
        self.batch_write_size.store(new_size, Ordering::SeqCst);
    }

    fn amount_to_change(&self) -> usize {
        self.current_size() / 10
    }
}

const fn eighty_percent_of(d: Duration) -> Duration {
    let millis = d.as_millis() as u64;
    Duration::from_millis(millis - (millis / 5))
}
