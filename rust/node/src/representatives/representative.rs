use std::sync::Arc;

use rsnano_core::Account;

use crate::transport::ChannelEnum;

#[derive(Clone)]
pub struct Representative {
    account: Account,
    channel: Arc<ChannelEnum>,
    last_request: u64,
    last_response: u64,
}

impl Representative {
    pub fn new(account: Account, channel: Arc<ChannelEnum>) -> Self {
        Self {
            account,
            channel,
            last_request: 0,
            last_response: 0,
        }
    }

    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn channel(&self) -> &Arc<ChannelEnum> {
        &self.channel
    }

    pub fn set_channel(&mut self, channel: Arc<ChannelEnum>) {
        self.channel = channel;
    }

    pub fn last_request(&self) -> u64 {
        self.last_request
    }

    pub fn set_last_request(&mut self, value: u64) {
        self.last_request = value
    }

    pub fn last_response(&mut self) -> u64 {
        self.last_response
    }

    pub fn set_last_response(&mut self, value: u64) {
        self.last_response = value
    }
}
