use crate::transport::ChannelEnum;
#[cfg(test)]
use mock_instant::Instant;
use rsnano_core::Account;
use std::sync::Arc;
#[cfg(not(test))]
use std::time::Instant;

#[derive(Clone)]
pub struct Representative {
    pub account: Account,
    pub channel: Arc<ChannelEnum>,
    pub last_request: Instant,
    pub last_response: Instant,
}

impl Representative {
    pub fn new(account: Account, channel: Arc<ChannelEnum>) -> Self {
        Self {
            account,
            channel,
            last_request: Instant::now(),
            last_response: Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new_test_instance() -> Self {
        Self::new(Account::from(42), Arc::new(ChannelEnum::new_null()))
    }
}
