use crate::transport::ChannelEnum;
#[cfg(test)]
use mock_instant::Instant;
use rsnano_core::Account;
use std::sync::Arc;
#[cfg(not(test))]
use std::time::Instant;

/// A representative to which we have a direct connection
#[derive(Clone)]
pub struct PeeredRep {
    pub account: Account,
    pub channel: Arc<ChannelEnum>,
    pub last_request: Instant,
}

impl PeeredRep {
    pub fn new(account: Account, channel: Arc<ChannelEnum>) -> Self {
        Self {
            account,
            channel,
            last_request: Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new_test_instance() -> Self {
        Self::new(Account::from(42), Arc::new(ChannelEnum::new_null()))
    }
}
