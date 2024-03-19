use crate::transport::ChannelEnum;
use rsnano_core::Account;
use std::{sync::Arc, time::Instant};

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

    #[cfg(test)]
    pub(crate) fn create_test_instance() -> Self {
        Self::new(
            Account::from(42),
            Arc::new(ChannelEnum::create_test_instance()),
        )
    }
}
