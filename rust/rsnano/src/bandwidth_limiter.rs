use crate::token_bucket::TokenBucket;
pub struct BandwidthLimiter {
    bucket: TokenBucket,
}

impl BandwidthLimiter {
    pub fn new(limit_burst_ratio: f64, limit: usize) -> Self {
        Self {
            bucket: TokenBucket::new((limit as f64 * limit_burst_ratio) as usize, limit),
        }
    }

    pub fn should_drop(&mut self, message_size: usize) -> bool {
        !self.bucket.try_consume(message_size)
    }

    pub fn reset(&mut self, limit_burst_ratio: f64, limit: usize) {
        self.bucket
            .reset((limit as f64 * limit_burst_ratio) as usize, limit)
    }
}
