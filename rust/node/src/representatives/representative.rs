use std::{sync::Arc, time::SystemTime};

use rsnano_core::Account;

use crate::transport::ChannelEnum;

#[derive(Clone)]
pub struct Representative {
    account: Account,
    channel: Arc<ChannelEnum>,
    last_request: SystemTime,
    last_response: SystemTime,
}

impl Representative {
    pub fn new(account: Account, channel: Arc<ChannelEnum>) -> Self {
        Self {
            account,
            channel,
            last_request: SystemTime::now(),
            last_response: SystemTime::now(),
        }
    }

    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn channel(&self) -> &Arc<ChannelEnum> {
        &self.channel
    }

    pub fn set_channel(&mut self, channel: Arc<ChannelEnum>) -> Arc<ChannelEnum> {
        std::mem::replace(&mut self.channel, channel)
    }

    pub fn last_request(&self) -> SystemTime {
        self.last_request
    }

    pub fn set_last_request(&mut self, value: SystemTime) {
        self.last_request = value
    }

    pub fn last_response(&mut self) -> SystemTime {
        self.last_response
    }

    pub fn set_last_response(&mut self, value: SystemTime) {
        self.last_response = value
    }

    #[cfg(test)]
    pub(crate) fn create_test_instance() -> Self {
        Self::new(
            Account::from(42),
            Arc::new(ChannelEnum::create_test_instance()),
        )
    }
}
