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
    minimum_size: usize,
}

pub(crate) struct BatchWriteSizeManagerOptions {
    pub min_size: usize,
}

impl BatchWriteSizeManagerOptions {
    pub const DEFAULT_MIN_SIZE: usize = 16384;
}

impl Default for BatchWriteSizeManagerOptions {
    fn default() -> Self {
        Self {
            min_size: Self::DEFAULT_MIN_SIZE,
        }
    }
}

impl Default for BatchWriteSizeManager {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl BatchWriteSizeManager {
    const MAXIMUM_BATCH_WRITE_TIME: Duration = Duration::from_millis(250);

    const MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF: Duration =
        eighty_percent_of(Self::MAXIMUM_BATCH_WRITE_TIME);

    pub fn new(options: BatchWriteSizeManagerOptions) -> Self {
        Self {
            batch_write_size: Arc::new(AtomicUsize::new(options.min_size)),
            minimum_size: options.min_size,
        }
    }

    pub fn current_size(&self) -> usize {
        self.batch_write_size.load(Ordering::SeqCst)
    }

    /// Include a tolerance to save having to potentially wait on the block processor if the number of blocks to cement is only a bit higher than the max.
    pub fn current_size_with_tolerance(&self) -> usize {
        let size = self.current_size();
        size.checked_add(size / 10).unwrap_or(usize::MAX)
    }

    pub fn set_size(&self, size: usize) {
        self.batch_write_size.store(size, Ordering::SeqCst);
    }

    pub fn adjust_size(&self, cementation_time: Duration, batch_size: usize) {
        // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
        if cementation_time > Self::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce();
        } else if batch_size >= self.current_size()
            && cementation_time < Self::MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF
        {
            // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
            self.increase();
        }
    }

    fn increase(&self) {
        let new_size = self
            .batch_write_size
            .load(Ordering::SeqCst)
            .checked_add(self.amount_to_change())
            .unwrap_or(usize::MAX);

        self.batch_write_size.store(new_size, Ordering::SeqCst);
    }

    fn reduce(&self) {
        // Reduce (unless we have hit a floor)
        let new_size = max(
            self.minimum_size,
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
