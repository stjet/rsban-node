use super::{BootstrapClient, BootstrapConnections, BootstrapStrategy};
use crate::{
    bootstrap::{bootstrap_limits, BootstrapConnectionsExt, PullInfo},
    transport::{BufferDropPolicy, TrafficType},
    utils::ErrorCode,
    NetworkParams,
};
use primitive_types::U256;
use rsnano_core::{
    utils::{BufferReader, Deserialize},
    Account, BlockHash,
};
use rsnano_ledger::Ledger;
use rsnano_messages::{FrontierReq, Message};
use std::{
    collections::VecDeque,
    ops::Deref,
    sync::{Arc, Condvar, Mutex, Weak},
    time::Instant,
};
use tracing::debug;

/// Client side of a frontier request. Created to send and listen for frontier sequences from the server.
pub struct FrontierReqClient {
    data: Mutex<FrontierReqClientData>,
    connection: Arc<BootstrapClient>,
    connections: Arc<BootstrapConnections>,
    ledger: Arc<Ledger>,
    attempt: Option<Weak<BootstrapStrategy>>,
    network_params: NetworkParams,
    condition: Condvar,
}

struct FrontierReqClientData {
    current: Account,
    frontier: BlockHash,
    frontiers_age: u32,
    count_limit: u32,
    accounts: VecDeque<(Account, BlockHash)>,
    start_time: Instant,
    count: u32,
    last_account: Account,
    /// A very rough estimate of the cost of `bulk_push`ing missing blocks
    bulk_push_cost: u64,
    result: Option<bool>,
}

impl FrontierReqClientData {
    fn bulk_push_available(&self) -> bool {
        self.bulk_push_cost < bootstrap_limits::BULK_PUSH_COST_LIMIT
            && self.frontiers_age == u32::MAX
    }

    fn next(&mut self, ledger: &Ledger) {
        // Filling accounts deque to prevent often read transactions
        if self.accounts.is_empty() {
            let max_size = 128;
            let txn = ledger.read_txn();
            let mut it = ledger
                .store
                .account
                .begin_account(&txn, &(self.current.number() + 1).into());

            while let Some((account, info)) = it.current() {
                if self.accounts.len() == max_size {
                    break;
                }
                self.accounts.push_back((*account, info.head));
                it.next();
            }

            /* If loop breaks before max_size, then accounts_end () is reached. Add empty record */
            if self.accounts.len() != max_size {
                self.accounts
                    .push_back((Account::zero(), BlockHash::zero()));
            }
        }
        // Retrieving accounts from deque
        let (current, frontier) = self.accounts.pop_front().unwrap();
        self.current = current;
        self.frontier = frontier;
    }
}

const SIZE_FRONTIER: usize = 32 + 32; // Account + BlockHash

impl FrontierReqClient {
    pub fn new(
        connection: Arc<BootstrapClient>,
        ledger: Arc<Ledger>,
        network_params: NetworkParams,
        connections: Arc<BootstrapConnections>,
    ) -> Self {
        Self {
            connection,
            ledger,
            attempt: None,
            network_params,
            connections,
            condition: Condvar::new(),
            data: Mutex::new(FrontierReqClientData {
                current: Account::zero(),
                frontier: BlockHash::zero(),
                frontiers_age: u32::MAX,
                count_limit: u32::MAX,
                accounts: Default::default(),
                start_time: Instant::now(),
                count: 0,
                bulk_push_cost: 0,
                last_account: Account::MAX, // Using last possible account stop further frontier requests
                result: None,
            }),
        }
    }

    pub fn set_attempt(&mut self, attempt: Arc<BootstrapStrategy>) {
        self.attempt = Some(Arc::downgrade(&attempt));
    }

    pub fn get_result(&self) -> bool {
        let guard = self.data.lock().unwrap();
        if let Some(result) = guard.result {
            return result;
        }
        let guard = self
            .condition
            .wait_while(guard, |i| i.result.is_none())
            .unwrap();
        guard.result.unwrap()
    }

    pub fn set_result(&self, result: bool) {
        {
            let mut guard = self.data.lock().unwrap();
            guard.result = Some(result);
        }
        self.condition.notify_all();
    }

    fn unsynced(&self, data: &mut FrontierReqClientData, head: BlockHash, end: BlockHash) {
        let Some(attempt) = self.attempt.as_ref().unwrap().upgrade() else {
            return;
        };

        let BootstrapStrategy::Legacy(legacy) = attempt.deref() else {
            return;
        };

        if data.bulk_push_available() {
            legacy.add_bulk_push_target(&head, &end);
            if end.is_zero() {
                data.bulk_push_cost += 2;
            } else {
                data.bulk_push_cost += 1;
            }
        }
    }
}

pub trait FrontierReqClientExt {
    fn run(&self, start_account: &Account, frontiers_age: u32, count: u32);
    fn receive_frontier(&self);
    fn received_frontier(&self, ec: ErrorCode, size_a: usize);
}

impl FrontierReqClientExt for Arc<FrontierReqClient> {
    fn run(&self, start_account: &Account, frontiers_age: u32, count: u32) {
        let request = Message::FrontierReq(FrontierReq {
            start: if start_account.is_zero() || start_account.number() == U256::MAX {
                *start_account
            } else {
                (start_account.number() + 1).into()
            },
            age: frontiers_age,
            count,
            only_confirmed: false,
        });
        {
            let mut guard = self.data.lock().unwrap();
            guard.current = *start_account;
            guard.frontiers_age = frontiers_age;
            guard.count_limit = count;
            guard.next(&self.ledger); // Load accounts from disk
        }
        let this_l = Arc::clone(self);
        self.connection.send(
            &request,
            Some(Box::new(move |ec, _size| {
                if ec.is_ok() {
                    this_l.receive_frontier();
                } else {
                    debug!("Error while sending bootstrap request: {:?}", ec);
                }
            })),
            BufferDropPolicy::NoLimiterDrop,
            TrafficType::Generic,
        );
    }

    fn receive_frontier(&self) {
        let this_l = Arc::clone(self);
        self.connection.read_async(
            SIZE_FRONTIER,
            Box::new(move |ec, size| {
                // An issue with asio is that sometimes, instead of reporting a bad file descriptor during disconnect,
                // we simply get a size of 0.
                if size == SIZE_FRONTIER {
                    this_l.received_frontier(ec, size);
                } else {
                    debug!("Invalid size: expected {}, got {}", SIZE_FRONTIER, size);
                }
            }),
        );
    }

    fn received_frontier(&self, ec: ErrorCode, size_a: usize) {
        let Some(attempt) = self.attempt.as_ref().unwrap().upgrade() else {
            return;
        };

        let BootstrapStrategy::Legacy(legacy) = attempt.deref() else {
            return;
        };

        if ec.is_ok() {
            debug_assert_eq!(size_a, SIZE_FRONTIER);
            let buf = self.connection.receive_buffer();
            let mut guard = self.data.lock().unwrap();
            let mut stream = BufferReader::new(&buf);
            let account = Account::deserialize(&mut stream).unwrap();
            let latest = BlockHash::deserialize(&mut stream).unwrap();
            if guard.count == 0 {
                guard.start_time = Instant::now();
            }
            guard.count += 1;
            let time_span = guard.start_time.elapsed();

            let elapsed_sec = time_span
                .as_secs_f64()
                .max(bootstrap_limits::BOOTSTRAP_MINIMUM_ELAPSED_SECONDS_BLOCKRATE);

            let blocks_per_sec = guard.count as f64 / elapsed_sec;
            let age_factor = if guard.frontiers_age == u32::MAX {
                1.0_f64
            } else {
                1.5_f64
            }; // Allow slower frontiers receive for requests with age

            if elapsed_sec > bootstrap_limits::BOOTSTRAP_CONNECTION_WARMUP_TIME_SEC
                && blocks_per_sec * age_factor
                    < bootstrap_limits::BOOTSTRAP_MINIMUM_FRONTIER_BLOCKS_PER_SEC
            {
                debug!("Aborting frontier req because it was too slow: {} frontiers per second, last {}", blocks_per_sec, account.encode_account());

                guard.result = Some(true);
                drop(guard);
                self.condition.notify_all();
                return;
            }

            if legacy.attempt.should_log() {
                debug!(
                    "Received {} frontiers from {}",
                    guard.count,
                    self.connection.channel_string()
                );
            }

            if !account.is_zero() && guard.count <= guard.count_limit {
                guard.last_account = account;
                while !guard.current.is_zero() && guard.current < account {
                    // We know about an account they don't.
                    let frontier = guard.frontier;
                    self.unsynced(&mut guard, frontier, BlockHash::zero());
                    guard.next(&self.ledger);
                }
                if !guard.current.is_zero() {
                    if account == guard.current {
                        if latest == guard.frontier {
                            // In sync
                        } else {
                            if self.ledger.block_or_pruned_exists(&latest) {
                                // We know about a block they don't.
                                let frontier = guard.frontier;
                                self.unsynced(&mut guard, frontier, latest);
                            } else {
                                let pull = PullInfo {
                                    account_or_head: account.into(),
                                    head: latest,
                                    head_original: latest,
                                    end: guard.frontier,
                                    count: 0,
                                    attempts: 0,
                                    processed: 0,
                                    retry_limit: self.network_params.bootstrap.frontier_retry_limit,
                                    bootstrap_id: legacy.attempt.incremental_id,
                                };
                                legacy.add_frontier(&pull);
                                // Either we're behind or there's a fork we differ on
                                // Either way, bulk pushing will probably not be effective
                                guard.bulk_push_cost += 5;
                            }
                        }
                        guard.next(&self.ledger);
                    } else {
                        debug_assert!(account < guard.current);
                        let pull = PullInfo {
                            account_or_head: account.into(),
                            head: latest,
                            head_original: latest,
                            end: BlockHash::zero(),
                            count: 0,
                            attempts: 0,
                            processed: 0,
                            retry_limit: self.network_params.bootstrap.frontier_retry_limit,
                            bootstrap_id: legacy.attempt.incremental_id,
                        };
                        legacy.add_frontier(&pull);
                    }
                } else {
                    let pull = PullInfo {
                        account_or_head: account.into(),
                        head: latest,
                        head_original: latest,
                        end: BlockHash::zero(),
                        count: 0,
                        attempts: 0,
                        processed: 0,
                        retry_limit: self.network_params.bootstrap.frontier_retry_limit,
                        bootstrap_id: legacy.attempt.incremental_id,
                    };

                    legacy.add_frontier(&pull);
                }
                self.receive_frontier();
            } else {
                if guard.count <= guard.count_limit {
                    while !guard.current.is_zero() && guard.bulk_push_available() {
                        // We know about an account they don't.
                        let frontier = guard.frontier;
                        self.unsynced(&mut guard, frontier, BlockHash::zero());
                        guard.next(&self.ledger);
                    }
                    // Prevent new frontier_req requests
                    legacy.set_start_account(Account::MAX);
                    debug!("Bulk push cost: {}", guard.bulk_push_cost);
                } else {
                    // Set last processed account as new start target
                    legacy.set_start_account(guard.last_account);
                }
                self.connections
                    .pool_connection(Arc::clone(&self.connection), false, false);
                guard.result = Some(false);
                self.condition.notify_all();
            }
        } else {
            debug!("Error while receiving frontier: {:?}", ec);
        }
    }
}
