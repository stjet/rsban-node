use super::{
    ordered_blocking::{BlockingEntry, OrderedBlocking},
    ordered_priorities::OrderedPriorities,
    AccountSetsConfig,
};
use crate::{
    bootstrap::ascending::ordered_priorities::PriorityEntry,
    stats::{DetailType, StatType, Stats},
};
use ordered_float::OrderedFloat;
use rsnano_core::{Account, BlockHash};
use std::{
    cmp::min,
    sync::Arc,
    time::{Duration, Instant},
};

/// This struct tracks accounts various account sets which are shared among the multiple bootstrap threads
pub(crate) struct AccountSets {
    stats: Arc<Stats>,
    config: AccountSetsConfig,
    priorities: OrderedPriorities,
    blocking: OrderedBlocking,
}

impl AccountSets {
    pub const PRIORITY_INITIAL: OrderedFloat<f32> = OrderedFloat(8.0);
    pub const PRIORITY_INCREASE: OrderedFloat<f32> = OrderedFloat(2.0);
    pub const PRIORITY_DECREASE: OrderedFloat<f32> = OrderedFloat(0.5);
    pub const PRIORITY_MAX: OrderedFloat<f32> = OrderedFloat(32.0);
    pub const PRIORITY_CUTOFF: OrderedFloat<f32> = OrderedFloat(1.0);

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
                Some(min(prio * Self::PRIORITY_INCREASE, Self::PRIORITY_MAX))
            }) {
                self.priorities
                    .insert(PriorityEntry::new(*account, Self::PRIORITY_INITIAL));
                self.stats.inc(
                    StatType::BootstrapAscendingAccounts,
                    DetailType::PriorityInsert,
                );

                self.trim_overflow();
            }
        } else {
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::PrioritizeFailed,
            );
        }
    }

    fn priority_down(&mut self, account: &Account) {
        if !self.priorities.change_priority(account, |prio| {
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::Deprioritize,
            );

            let priority_new = prio - Self::PRIORITY_DECREASE;
            if priority_new <= Self::PRIORITY_CUTOFF {
                self.stats.inc(
                    StatType::BootstrapAscendingAccounts,
                    DetailType::PriorityEraseThreshold,
                );
                None
            } else {
                Some(priority_new)
            }
        }) {
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::DeprioritizeFailed,
            );
        }
    }

    fn block(&mut self, account: Account, dependency: BlockHash) {
        self.stats
            .inc(StatType::BootstrapAscendingAccounts, DetailType::Block);

        let entry = self.priorities.remove(&account).unwrap_or_default();
        self.stats.inc(
            StatType::BootstrapAscendingAccounts,
            DetailType::PriorityEraseBlock,
        );

        self.blocking.insert(BlockingEntry {
            account,
            dependency,
            original_entry: entry,
        });
        self.stats.inc(
            StatType::BootstrapAscendingAccounts,
            DetailType::BlockingInsert,
        );

        self.trim_overflow();
    }

    fn unblock(&mut self, account: Account, hash: Option<BlockHash>) {
        // Unblock only if the dependency is fulfilled

        if let Some(existing) = self.blocking.get(&account) {
            let hash_matches = if let Some(hash) = hash {
                hash == existing.dependency
            } else {
                true
            };

            if hash_matches {
                self.stats
                    .inc(StatType::BootstrapAscendingAccounts, DetailType::Unblock);

                debug_assert!(!self.priorities.contains(&account));
                if !existing.original_entry.account.is_zero() {
                    debug_assert!(existing.original_entry.account == account);
                    self.priorities.insert(existing.original_entry);
                } else {
                    self.priorities
                        .insert(PriorityEntry::new(account, Self::PRIORITY_INITIAL));
                }
                self.blocking.remove(&account);

                self.trim_overflow();
                return;
            }
        }
        self.stats.inc(
            StatType::BootstrapAscendingAccounts,
            DetailType::UnblockFailed,
        );
    }

    fn timestamp(&mut self, account: &Account, reset: bool) {
        let tstamp = if reset { None } else { Some(Instant::now()) };
        self.priorities.change_timestamp(account, tstamp);
    }

    fn check_timestamp(&self, account: &Account) -> bool {
        if let Some(entry) = self.priorities.get(account) {
            if entry
                .timestamp
                .map(|i| i.elapsed())
                .unwrap_or(Duration::MAX)
                < self.config.cooldown
            {
                return false;
            }
        }

        true
    }

    fn trim_overflow(&mut self) {
        if self.priorities.len() > self.config.priorities_max {
            self.priorities.pop_lowest_priority();
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::PriorityEraseOverflow,
            );
        }
        if self.blocking.len() > self.config.blocking_max {
            // Evict the lowest priority entry
            self.blocking.pop_lowest_priority();

            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::BlockingEraseOverflow,
            );
        }
    }

    //nano::account nano::bootstrap_ascending::account_sets::next ()
    //{
    //	if (priorities.empty ())
    //	{
    //		return { 0 };
    //	}
    //
    //	std::vector<float> weights;
    //	std::vector<nano::account> candidates;
    //
    //	int iterations = 0;
    //	while (candidates.size () < config.consideration_count && iterations++ < config.consideration_count * 10)
    //	{
    //		debug_assert (candidates.size () == weights.size ());
    //
    //		// Use a dedicated, uniformly distributed field for sampling to avoid problematic corner case when accounts in the queue are very close together
    //		auto search = nano::bootstrap_ascending::generate_id ();
    //		auto iter = priorities.get<tag_id> ().lower_bound (search);
    //		if (iter == priorities.get<tag_id> ().end ())
    //		{
    //			iter = priorities.get<tag_id> ().begin ();
    //		}
    //
    //		if (check_timestamp (iter->account))
    //		{
    //			candidates.push_back (iter->account);
    //			weights.push_back (iter->priority);
    //		}
    //	}
    //
    //	if (candidates.empty ())
    //	{
    //		return { 0 }; // All sampled accounts are busy
    //	}
    //
    //	std::discrete_distribution dist{ weights.begin (), weights.end () };
    //	auto selection = dist (rng);
    //	debug_assert (!weights.empty () && selection < weights.size ());
    //	auto result = candidates[selection];
    //	return result;
    //}

    fn blocked(&self, account: &Account) -> bool {
        self.blocking.contains(account)
    }

    //std::size_t nano::bootstrap_ascending::account_sets::priority_size () const
    //{
    //	return priorities.size ();
    //}
    //
    //std::size_t nano::bootstrap_ascending::account_sets::blocked_size () const
    //{
    //	return blocking.size ();
    //}
    //
    //float nano::bootstrap_ascending::account_sets::priority (nano::account const & account) const
    //{
    //	if (blocked (account))
    //	{
    //		return 0.0f;
    //	}
    //	auto existing = priorities.get<tag_account> ().find (account);
    //	if (existing != priorities.get<tag_account> ().end ())
    //	{
    //		return existing->priority;
    //	}
    //	return account_sets::priority_cutoff;
    //}
    //
    //auto nano::bootstrap_ascending::account_sets::info () const -> nano::bootstrap_ascending::account_sets::info_t
    //{
    //	return { blocking, priorities };
    //}
    //
    //std::unique_ptr<nano::container_info_component> nano::bootstrap_ascending::account_sets::collect_container_info (const std::string & name)
    //{
    //	auto composite = std::make_unique<container_info_composite> (name);
    //	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "priorities", priorities.size (), sizeof (decltype (priorities)::value_type) }));
    //	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "blocking", blocking.size (), sizeof (decltype (blocking)::value_type) }));
    //	return composite;
    //}
}
