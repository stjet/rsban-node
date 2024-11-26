use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, BlockHash, PendingKey,
};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::LmdbReadTransaction;
use std::{collections::VecDeque, sync::Arc};

const BATCH_SIZE: usize = 512;

pub(crate) struct DatabaseScan {
    queue: VecDeque<Account>,
    accounts_iterator: AccountDatabaseIterator,
    pending_iterator: PendingDatabaseIterator,
    ledger: Arc<Ledger>,
}

impl DatabaseScan {
    pub fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            accounts_iterator: AccountDatabaseIterator::new(ledger.clone()),
            pending_iterator: PendingDatabaseIterator::new(ledger.clone()),
            ledger,
            queue: Default::default(),
        }
    }

    pub fn next(&mut self, filter: impl Fn(&Account) -> bool) -> Account {
        if self.queue.is_empty() {
            self.fill();
        }

        while let Some(result) = self.queue.pop_front() {
            if filter(&result) {
                return result;
            }
        }

        Account::zero()
    }

    fn fill(&mut self) {
        let tx = self.ledger.read_txn();
        let set1 = self.accounts_iterator.next_batch(&tx, BATCH_SIZE);
        let set2 = self.pending_iterator.next_batch(&tx, BATCH_SIZE);
        self.queue.extend(set1);
        self.queue.extend(set2);
    }

    pub fn warmed_up(&self) -> bool {
        self.accounts_iterator.warmed_up() && self.pending_iterator.warmed_up()
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "accounts_iterator".to_owned(),
                    count: self.accounts_iterator.completed,
                    sizeof_element: 0,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "pending_iterator".to_owned(),
                    count: self.pending_iterator.completed,
                    sizeof_element: 0,
                }),
            ],
        )
    }
}

struct AccountDatabaseIterator {
    ledger: Arc<Ledger>,
    next: Account,
    completed: usize,
}

impl AccountDatabaseIterator {
    fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            ledger,
            next: Account::zero(),
            completed: 0,
        }
    }

    fn next_batch(&mut self, tx: &LmdbReadTransaction, batch_size: usize) -> Vec<Account> {
        let mut result = Vec::new();
        let accounts = self.ledger.store.account.iter_range(tx, self.next..);
        let count = 0;
        let mut end_reached = true;
        for (account, _) in accounts {
            if count >= batch_size {
                end_reached = false;
                break;
            }
            result.push(account);
            self.next = account.inc().unwrap_or_default();
        }

        if end_reached {
            // Reset for the next ledger iteration
            self.next = Account::zero();
            self.completed += 1;
        }

        result
    }

    fn warmed_up(&self) -> bool {
        self.completed > 0
    }
}

struct PendingDatabaseIterator {
    ledger: Arc<Ledger>,
    next: PendingKey,
    completed: usize,
}

impl PendingDatabaseIterator {
    fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            ledger,
            next: PendingKey::default(),
            completed: 0,
        }
    }

    fn next_batch(&mut self, tx: &LmdbReadTransaction, batch_size: usize) -> Vec<Account> {
        let mut result = Vec::new();
        let mut it = self.ledger.store.pending.begin_at_key(tx, &self.next);
        // TODO: This pending iteration heuristic should be encapsulated in a pending_iterator class and reused across other components
        // The heuristic is to advance the iterator sequentially until we reach a new account or perform a fresh lookup if the account has too many pending blocks
        // This is to avoid the overhead of performing a fresh lookup for every pending account as majority of accounts have only a few pending blocks

        let mut count = 0;
        let mut end_reached = true;
        while let Some((key, _)) = it.current() {
            if count >= batch_size {
                end_reached = false;
                break;
            }
            result.push(key.receiving_account);
            self.next = PendingKey::new(
                key.receiving_account.inc().unwrap_or_default(),
                BlockHash::zero(),
            );
            count += 1;

            // advance iterator:
            let starting_account = key.receiving_account;

            // For RocksDB, sequential access is ~10x faster than performing a fresh lookup (tested on my machine)
            const SEQUENTIAL_ATTEMPTS: usize = 10;

            for _ in 0..SEQUENTIAL_ATTEMPTS {
                let current = it.current();
                let Some(current) = &current else {
                    break;
                };
                if current.0.receiving_account != starting_account {
                    break;
                }
                it.next();
            }

            // If we didn't advance to the next account, perform a fresh lookup
            if let Some(current) = it.current() {
                if current.0.receiving_account != starting_account {
                    it = self.ledger.store.pending.begin_at_key(
                        tx,
                        &PendingKey::new(
                            starting_account.inc().unwrap_or_default(),
                            BlockHash::zero(),
                        ),
                    );
                }
            }
            debug_assert!(
                it.is_end() || it.current().unwrap().0.receiving_account != starting_account
            );
        }

        if end_reached {
            // Reset for the next ledger iteration
            self.next = Default::default();
            self.completed += 1;
        }
        result
    }

    fn warmed_up(&self) -> bool {
        self.completed > 0
    }
}
