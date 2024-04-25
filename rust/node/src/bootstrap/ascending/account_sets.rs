use ordered_float::OrderedFloat;
use rsnano_core::Account;

use super::{
    ordered_blocking::OrderedBlocking, ordered_priorities::OrderedPriorities, AccountSetsConfig,
};
use crate::{
    bootstrap::ascending::ordered_priorities::PriorityEntry,
    stats::{DetailType, StatType, Stats},
};
use std::{cmp::min, sync::Arc};

/// This struct tracks accounts various account sets which are shared among the multiple bootstrap threads
pub(crate) struct AccountSets {
    stats: Arc<Stats>,
    config: AccountSetsConfig,
    priorities: OrderedPriorities,
    blocking: OrderedBlocking,
}

impl AccountSets {
    pub const PRIORITY_INCREASE: OrderedFloat<f32> = OrderedFloat(2.0);
    pub const PRIORITY_MAX: OrderedFloat<f32> = OrderedFloat(32.0);
    pub const PRIORITY_INITIAL: OrderedFloat<f32> = OrderedFloat(8.0);

    pub fn new(stats: Arc<Stats>, config: AccountSetsConfig) -> Self {
        Self {
            stats,
            config,
            priorities: Default::default(),
            blocking: Default::default(),
        }
    }

    fn priority_up(&mut self, account: &Account) {
        if !self.blocked(account) {
            self.stats
                .inc(StatType::BootstrapAscendingAccounts, DetailType::Prioritize);

            if !self.priorities.change_priority(account, |prio| {
                min(prio * Self::PRIORITY_INCREASE, Self::PRIORITY_MAX)
            }) {
                self.priorities
                    .insert(PriorityEntry::new(*account, Self::PRIORITY_INITIAL));
                //        stats.inc (nano::stat::type::bootstrap_ascending_accounts, nano::stat::detail::priority_insert);

                //        trim_overflow ();
            }
        } else {
            //    stats.inc (nano::stat::type::bootstrap_ascending_accounts, nano::stat::detail::prioritize_failed);
        }
        todo!()
    }

    fn blocked(&self, account: &Account) -> bool {
        self.blocking.contains(account)
    }
}
