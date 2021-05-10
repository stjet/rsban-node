use std::time::{Duration, Instant};
use std::sync::Mutex;

pub struct TokenBucket{
    last_refill: Instant,
    current_size: usize,
    max_token_count: usize,

    /** The minimum observed bucket size, from which the largest burst can be derived */
    smallest_size: usize,
    refill_rate: usize,
}

const UNLIMITED: usize = 1_000_000_000;

impl TokenBucket{
    pub fn new() -> Self {
        Self{
            last_refill: Instant::now(),
            current_size: 0,
            max_token_count: 0,
            smallest_size: 0,
            refill_rate: 0
        }
    }

    pub fn reset(&mut self, mut max_token_count: usize, mut refill_rate: usize){
        // A token count of 0 indicates unlimited capacity. We use 1e9 as
        // a sentinel, allowing largest burst to still be computed.
        if max_token_count == 0 || refill_rate == 0
        {
            refill_rate = UNLIMITED;
            max_token_count = UNLIMITED;
        }
        self.smallest_size = max_token_count;
        self.max_token_count = max_token_count;
        self.current_size = max_token_count;
        self.refill_rate = refill_rate;
        self.last_refill = Instant::now()
    }

    fn elapsed(&mut self) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        self.last_refill = now;
        elapsed
    }

    pub fn largest_burst (&self) -> usize {
        self.max_token_count - self.smallest_size
    }

    fn refill(&mut self)
    {
        let tokens_to_add = ( self.elapsed().as_nanos() as f64 / 1e9 as f64 * self.refill_rate as f64) as usize;
        self.current_size = std::cmp::min (self.current_size + tokens_to_add, self.max_token_count);
    }

    pub fn try_consume (&mut self, tokens_required: u32) -> bool
    {
        debug_assert!(tokens_required as usize <= UNLIMITED);
        self.refill();
        let possible = self.current_size >= tokens_required as usize;
        if possible {
            self.current_size = self.current_size - tokens_required as usize;
        }
        else if tokens_required as usize == UNLIMITED
        {
            self.current_size = 0;
        }

        // Keep track of smallest observed bucket size so burst size can be computed (for tests and stats)
        self.smallest_size = std::cmp::min(self.smallest_size, self.current_size);

        return possible || self.refill_rate == UNLIMITED;
    }
}

#[no_mangle]
pub extern "C" fn rsn_token_bucket_create() -> *mut Mutex<TokenBucket> {
    Box::into_raw(Box::new(Mutex::new(TokenBucket::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_destroy(bucket: *mut Mutex<TokenBucket>) {
    drop(Box::from_raw(bucket));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_reset(bucket: &mut Mutex<TokenBucket>, max_token_count: usize, refill_rate: usize) {
    bucket.lock().unwrap().reset(max_token_count, refill_rate);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_largest_burst(bucket: &mut Mutex<TokenBucket>) -> usize {
    bucket.lock().unwrap().largest_burst()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_try_consume(bucket: &mut Mutex<TokenBucket>, tokens_required: u32) -> bool {
    bucket.lock().unwrap().try_consume(tokens_required)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
