use crate::transport::ChannelId;
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
    pub channel_id: ChannelId,
    pub last_request: Instant,
}

impl PeeredRep {
    pub fn new(account: Account, channel_id: ChannelId) -> Self {
        Self {
            account,
            channel_id,
            last_request: Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new_test_instance() -> Self {
        Self::new(Account::from(42), 123.into())
    }
}
