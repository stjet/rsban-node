use rsnano_core::Account;

use super::Representative;
use crate::transport::ChannelEnum;
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct RepresentativeRegister {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
    last_requests: Vec<Account>,
}

pub enum RegisterRepresentativeResult {
    Inserted,
    Updated,
    ChannelChanged(Arc<ChannelEnum>),
}

impl RepresentativeRegister {
    pub fn new() -> Self {
        Default::default()
    }

    /// Returns the old channel if the representative was already in the collection
    pub fn update_or_insert(
        &mut self,
        account: Account,
        channel: Arc<ChannelEnum>,
    ) -> RegisterRepresentativeResult {
        todo!()
        //if let Some(rep) = self.by_account.get_mut(&account) {
        //    rep.set_last_response(value)
        //} else {
        //    self.by_account
        //        .insert(account, Representative::new(account, channel));

        //    let by_id = self
        //        .by_channel_id
        //        .entry(channel.as_channel().channel_id())
        //        .or_default();

        //    by_id.push(account);
        //    self.last_requests.push(account);
        //    RegisterRepresentativeResult::Inserted
        //}
    }
}
