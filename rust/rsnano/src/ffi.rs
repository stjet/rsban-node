use crate::token_bucket::TokenBucket;
use std::sync::Mutex;

#[no_mangle]
pub extern "C" fn rsn_token_bucket_create(
    max_token_count: usize,
    refill_rate: usize,
) -> *mut Mutex<TokenBucket> {
    Box::into_raw(Box::new(Mutex::new(TokenBucket::new(
        max_token_count,
        refill_rate,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_destroy(bucket: *mut Mutex<TokenBucket>) {
    drop(Box::from_raw(bucket));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_reset(
    bucket: &Mutex<TokenBucket>,
    max_token_count: usize,
    refill_rate: usize,
) {
    bucket.lock().unwrap().reset(max_token_count, refill_rate);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_largest_burst(bucket: &Mutex<TokenBucket>) -> usize {
    bucket.lock().unwrap().largest_burst()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_token_bucket_try_consume(
    bucket: &Mutex<TokenBucket>,
    tokens_required: usize,
) -> bool {
    bucket.lock().unwrap().try_consume(tokens_required)
}
