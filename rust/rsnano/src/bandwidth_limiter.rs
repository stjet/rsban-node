use crate::TokenBucket;
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

#[cfg(test)]
mod tests {
    use super::*;
    use mock_instant::MockClock;
    use std::time::Duration;

    #[test]
    fn test_limit() {
        let mut limiter = BandwidthLimiter::new(1.5, 10);
        assert_eq!(limiter.should_drop(15), false);
        assert_eq!(limiter.should_drop(1), true);
        MockClock::advance(Duration::from_millis(100));
        assert_eq!(limiter.should_drop(1), false);
        assert_eq!(limiter.should_drop(1), true);
    }
}
