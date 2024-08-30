use super::{
    ordered_blocking::{BlockingEntry, OrderedBlocking},
    ordered_priorities::{OrderedPriorities, Priority},
};
use crate::{
    bootstrap::ascending::ordered_priorities::PriorityEntry,
    stats::{DetailType, StatType, Stats},
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, BlockHash,
};
use std::{
    cmp::min,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub struct AccountSetsConfig {
    pub consideration_count: usize,
    pub priorities_max: usize,
    pub blocking_max: usize,
    pub cooldown: Duration,
}

impl Default for AccountSetsConfig {
    fn default() -> Self {
        Self {
            consideration_count: 4,
            priorities_max: 256 * 1024,
            blocking_max: 256 * 1024,
            cooldown: Duration::from_secs(3),
        }
    }
}

/// This struct tracks accounts various account sets which are shared among the multiple bootstrap threads
pub(crate) struct AccountSets {
    stats: Arc<Stats>,
    config: AccountSetsConfig,
    priorities: OrderedPriorities,
    blocking: OrderedBlocking,
}

impl AccountSets {
    pub const PRIORITY_INITIAL: Priority = Priority::new(2.0);
    pub const PRIORITY_INCREASE: Priority = Priority::new(2.0);
    pub const PRIORITY_DIVIDE: f64 = 2.0;
    pub const PRIORITY_MAX: Priority = Priority::new(128.0);
    pub const PRIORITY_CUTOFF: Priority = Priority::new(0.15);

    pub fn new(config: AccountSetsConfig, stats: Arc<Stats>) -> Self {
        Self {
            stats,
            config,
            priorities: Default::default(),
            blocking: Default::default(),
        }
    }

    /**
     * If an account is not blocked, increase its priority.
     * If the account does not exist in priority set and is not blocked, inserts a new entry.
     * Current implementation increases priority by 1.0f each increment
     */
    pub fn priority_up(&mut self, account: &Account) {
        if account.is_zero() {
            return;
        }

        if !self.blocked(account) {
            self.stats
                .inc(StatType::BootstrapAscendingAccounts, DetailType::Prioritize);

            if !self.priorities.change_priority(account, |prio| {
                Some(min(prio + Self::PRIORITY_INCREASE, Self::PRIORITY_MAX))
            }) {
                self.stats.inc(
                    StatType::BootstrapAscendingAccounts,
                    DetailType::PriorityInsert,
                );

                self.priorities
                    .insert(PriorityEntry::new(*account, Self::PRIORITY_INITIAL));

                self.trim_overflow();
            }
        } else {
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::PrioritizeFailed,
            );
        }
    }

    /**
     * Decreases account priority
     * Current implementation divides priority by 2.0f and saturates down to 1.0f.
     */
    pub fn priority_down(&mut self, account: &Account) {
        if account.is_zero() {
            return;
        }

        if !self.priorities.change_priority(account, |prio| {
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::Deprioritize,
            );

            let priority_new = prio / Self::PRIORITY_DIVIDE;
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

    pub fn priority_set(&mut self, account: &Account) {
        Self::priority_set_impl(account, &self.stats, &self.blocking, &mut self.priorities);
        self.trim_overflow();
    }

    fn priority_set_impl(
        account: &Account,
        stats: &Stats,
        blocking: &OrderedBlocking,
        priorities: &mut OrderedPriorities,
    ) {
        if account.is_zero() {
            return;
        }

        if !blocking.contains(account) {
            if !priorities.contains(account) {
                stats.inc(
                    StatType::BootstrapAscendingAccounts,
                    DetailType::PriorityInsert,
                );
                priorities.insert(PriorityEntry::new(*account, Self::PRIORITY_INITIAL));
            }
        } else {
            stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::PrioritizeFailed,
            );
        }
    }

    pub fn block(&mut self, account: Account, dependency: BlockHash) {
        debug_assert!(!account.is_zero());

        self.stats
            .inc(StatType::BootstrapAscendingAccounts, DetailType::Block);

        let entry = self
            .priorities
            .remove(&account)
            .unwrap_or_else(|| PriorityEntry::new(account, Priority::ZERO));

        self.stats.inc(
            StatType::BootstrapAscendingAccounts,
            DetailType::PriorityEraseBlock,
        );

        self.blocking.insert(BlockingEntry {
            dependency,
            dependency_account: Account::zero(), //TODO
            original_entry: entry,
        });
        self.stats.inc(
            StatType::BootstrapAscendingAccounts,
            DetailType::BlockingInsert,
        );

        self.trim_overflow();
    }

    pub fn unblock(&mut self, account: Account, hash: Option<BlockHash>) {
        if account.is_zero() {
            return;
        }

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
                    self.priorities.insert(existing.original_entry.clone());
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

    pub fn timestamp_set(&mut self, account: &Account) {
        debug_assert!(!account.is_zero());

        let tstamp = Instant::now();
        self.priorities.change_timestamp(account, Some(tstamp));
    }

    pub fn timestamp_reset(&mut self, account: &Account) {
        debug_assert!(!account.is_zero());

        self.priorities.change_timestamp(account, None);
    }

    /// Sets information about the account chain that contains the block hash
    pub fn dependency_update(&mut self, dependency: &BlockHash, dependency_account: Account) {
        debug_assert!(!dependency_account.is_zero());
        let updated = self
            .blocking
            .modify_dependency_account(dependency, dependency_account);
        if updated > 0 {
            self.stats.add(
                StatType::BootstrapAscendingAccounts,
                DetailType::DependencyUpdate,
                updated as u64,
            );
        } else {
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::DependencyUpdateFailed,
            );
        }
    }

    fn trim_overflow(&mut self) {
        while self.priorities.len() > self.config.priorities_max {
            // Erase the oldest entry
            self.priorities.pop_front();
            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::PriorityEraseOverflow,
            );
        }
        while self.blocking.len() > self.config.blocking_max {
            // Erase the oldest entry
            self.blocking.pop_front();

            self.stats.inc(
                StatType::BootstrapAscendingAccounts,
                DetailType::BlockingEraseOverflow,
            );
        }
    }

    /// Sampling
    pub fn next_priority(&self, filter: impl Fn(&Account) -> bool) -> Account {
        if self.priorities.is_empty() {
            return Account::zero();
        }

        let cutoff = Instant::now() - self.config.cooldown;

        self.priorities
            .next_priority(cutoff, filter)
            .unwrap_or_default()
    }

    pub fn next_blocking(&self, filter: impl Fn(&BlockHash) -> bool) -> BlockHash {
        if self.blocking.len() == 0 {
            return BlockHash::zero();
        }

        self.blocking.next(filter).unwrap_or_default()
    }

    /// Sets information about the account chain that contains the block hash
    pub fn sync_dependencies(&mut self) {
        // Sample all accounts with a known dependency account (> account 0)
        let begin = Account::zero().inc().unwrap();
        for entry in self.blocking.iter_start_dep_account(begin) {
            if self.priorities.len() >= self.config.priorities_max {
                break;
            }

            if !self.blocked(&entry.dependency_account)
                && !self.prioritized(&entry.dependency_account)
            {
                self.stats.inc(
                    StatType::BootstrapAscendingAccounts,
                    DetailType::SyncDependencies,
                );
                Self::priority_set_impl(
                    &entry.dependency_account,
                    &self.stats,
                    &self.blocking,
                    &mut self.priorities,
                );
            }
        }

        self.trim_overflow();
    }

    fn blocked(&self, account: &Account) -> bool {
        self.blocking.contains(account)
    }

    fn prioritized(&self, account: &Account) -> bool {
        self.priorities.contains(account)
    }

    pub fn priority_len(&self) -> usize {
        self.priorities.len()
    }

    pub fn blocked_len(&self) -> usize {
        self.blocking.len()
    }

    pub fn priority_half_full(&self) -> bool {
        self.priorities.len() > self.config.priorities_max / 2
    }

    pub fn blocked_half_full(&self) -> bool {
        self.blocking.len() > self.config.blocking_max / 2
    }

    /// Accounts in the ledger but not in priority list are assumed priority 1.0f
    /// Blocked accounts are assumed priority 0.0f
    #[allow(dead_code)]
    pub fn priority(&self, account: &Account) -> Priority {
        if !self.blocked(account) {
            if let Some(existing) = self.priorities.get(account) {
                return existing.priority;
            }
        }
        return Priority::ZERO;
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        // Count blocking entries with their dependency account unknown
        let blocking_unknown = self.blocking.count_by_dependency_account(&Account::zero());

        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "priorities".to_string(),
                    count: self.priorities.len(),
                    sizeof_element: OrderedPriorities::ELEMENT_SIZE,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "blocking".to_string(),
                    count: self.blocking.len(),
                    sizeof_element: OrderedBlocking::ELEMENT_SIZE,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "blocking_unknown".to_string(),
                    count: blocking_unknown,
                    sizeof_element: 0,
                }),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_blocked() {
        fixture(|sets| {
            assert_eq!(sets.blocked(&Account::from(1)), false);
        });
    }

    #[test]
    fn block() {
        fixture(|sets| {
            let account = Account::from(1);
            let hash = BlockHash::from(2);

            sets.block(account, hash);

            assert!(sets.blocked(&account));
            assert_eq!(sets.priority(&account), Priority::ZERO);
        });
    }

    #[test]
    fn unblock() {
        fixture(|sets| {
            let account = Account::from(1);
            let hash = BlockHash::from(2);

            sets.block(account, hash);
            sets.unblock(account, None);

            assert_eq!(sets.blocked(&account), false);
        });
    }

    #[test]
    fn priority_base() {
        fixture(|sets| {
            assert_eq!(sets.priority(&Account::from(1)), Priority::ZERO);
        });
    }

    // When account is unblocked, check that it retains it former priority
    #[test]
    fn priority_unblock_keep() {
        fixture(|sets| {
            let account = Account::from(1);
            let hash = BlockHash::from(2);

            sets.priority_up(&account);
            sets.priority_up(&account);

            sets.block(account, hash);
            sets.unblock(account, None);

            assert_eq!(sets.priority(&account), Priority::new(4.0));
        });
    }

    #[test]
    fn priority_up_down() {
        fixture(|sets| {
            let account = Account::from(1);

            sets.priority_up(&account);
            assert_eq!(sets.priority(&account), AccountSets::PRIORITY_INITIAL);

            sets.priority_down(&account);
            assert_eq!(sets.priority(&account), Priority::new(1.0));
        });
    }

    // Check that priority downward saturates to 1.0f
    #[test]
    fn priority_down_saturates() {
        fixture(|sets| {
            let account = Account::from(1);

            sets.priority_down(&account);
            assert_eq!(sets.priority(&account), Priority::ZERO);
        });
    }

    // Ensure priority value is bounded
    #[test]
    fn saturate_priority() {
        fixture(|sets| {
            let account = Account::from(1);

            for _ in 0..100 {
                sets.priority_up(&account);
            }
            assert_eq!(sets.priority(&account), AccountSets::PRIORITY_MAX);
        });
    }

    fn fixture(mut f: impl FnMut(&mut AccountSets)) {
        let stats = Arc::new(Stats::default());
        let config = AccountSetsConfig::default();
        let mut sets = AccountSets::new(config, stats);
        f(&mut sets);
    }
}
