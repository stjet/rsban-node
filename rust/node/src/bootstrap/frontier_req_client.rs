use super::{BootstrapAttemptLegacy, BootstrapClient};
use crate::{
    bootstrap::bootstrap_limits,
    transport::{BufferDropPolicy, TrafficType},
    utils::ErrorCode,
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
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex, Weak,
    },
    time::Instant,
};
use tracing::debug;

pub struct FrontierReqClient {
    data: Mutex<FrontierReqClientData>,
    connection: Arc<BootstrapClient>,
    ledger: Arc<Ledger>,
    attempt: Mutex<Option<Weak<BootstrapAttemptLegacy>>>,
    tx: Sender<bool>,
    rx: Receiver<bool>,
}

struct FrontierReqClientData {
    current: Account,
    frontier: BlockHash,
    frontiers_age: u32,
    count_limit: u32,
    accounts: VecDeque<(Account, BlockHash)>,
    start_time: Instant,
    count: u32,
}

const SIZE_FRONTIER: usize = 32 + 32; // Account + BlockHash

impl FrontierReqClient {
    pub fn new(connection: Arc<BootstrapClient>, ledger: Arc<Ledger>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            connection,
            ledger,
            tx,
            rx,
            attempt: Mutex::new(None),
            data: Mutex::new(FrontierReqClientData {
                current: Account::zero(),
                frontier: BlockHash::zero(),
                frontiers_age: u32::MAX,
                count_limit: u32::MAX,
                accounts: Default::default(),
                start_time: Instant::now(),
                count: 0,
            }),
        }
    }

    fn next(&self) {
        let mut guard = self.data.lock().unwrap();
        // Filling accounts deque to prevent often read transactions
        if guard.accounts.is_empty() {
            let max_size = 128;
            let txn = self.ledger.read_txn();
            let mut it = self
                .ledger
                .store
                .account
                .begin_account(&txn, &(guard.current.number() + 1).into());

            while let Some((account, info)) = it.current() {
                if guard.accounts.len() == max_size {
                    break;
                }
                guard.accounts.push_back((*account, info.head));
                it.next();
            }

            /* If loop breaks before max_size, then accounts_end () is reached. Add empty record */
            if guard.accounts.len() != max_size {
                guard
                    .accounts
                    .push_back((Account::zero(), BlockHash::zero()));
            }
        }
        // Retrieving accounts from deque
        let (current, frontier) = guard.accounts.pop_front().unwrap();
        guard.current = current;
        guard.frontier = frontier;
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
        }
        self.next(); // Load accounts from disk
        let this_l = Arc::clone(self);
        self.connection.send(
            &request,
            Some(Box::new(move |ec, size| {
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
        let Some(attempt) = self.attempt.lock().unwrap().as_ref().unwrap().upgrade() else {
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

                self.tx.send(true);
                return;
            }

            if attempt.should_log() {
                debug!(
                    "Received {} frontiers from {}",
                    guard.count,
                    self.connection.channel_string()
                );
            }

            if !account.is_zero() && guard.count <= count_limit {
                //		last_account = account;
                //		while (!current.is_zero () && current < account)
                //		{
                //			// We know about an account they don't.
                //			unsynced (frontier, 0);
                //			next ();
                //		}
                //		if (!current.is_zero ())
                //		{
                //			if (account == current)
                //			{
                //				if (latest == frontier)
                //				{
                //					// In sync
                //				}
                //				else
                //				{
                //					if (node->ledger.block_or_pruned_exists (latest))
                //					{
                //						// We know about a block they don't.
                //						unsynced (frontier, latest);
                //					}
                //					else
                //					{
                //						attempt_l->add_frontier (nano::pull_info (account, latest, frontier, attempt_l->get_incremental_id (), 0, node->network_params.bootstrap.frontier_retry_limit));
                //						// Either we're behind or there's a fork we differ on
                //						// Either way, bulk pushing will probably not be effective
                //						bulk_push_cost += 5;
                //					}
                //				}
                //				next ();
                //			}
                //			else
                //			{
                //				debug_assert (account < current);
                //				attempt_l->add_frontier (nano::pull_info (account, latest, nano::block_hash (0), attempt_l->get_incremental_id (), 0, node->network_params.bootstrap.frontier_retry_limit));
                //			}
                //		}
                //		else
                //		{
                //			attempt_l->add_frontier (nano::pull_info (account, latest, nano::block_hash (0), attempt_l->get_incremental_id (), 0, node->network_params.bootstrap.frontier_retry_limit));
                //		}
                //		receive_frontier ();
            } else {
                //		if (count <= count_limit)
                //		{
                //			while (!current.is_zero () && bulk_push_available ())
                //			{
                //				// We know about an account they don't.
                //				unsynced (frontier, 0);
                //				next ();
                //			}
                //			// Prevent new frontier_req requests
                //			attempt_l->set_start_account (std::numeric_limits<nano::uint256_t>::max ());
                //			node->logger->debug (nano::log::type::frontier_req_client, "Bulk push cost: {}", bulk_push_cost);
                //		}
                //		else
                //		{
                //			// Set last processed account as new start target
                //			attempt_l->set_start_account (last_account);
                //		}
                //		node->bootstrap_initiator.connections->pool_connection (connection);
                //		try
                //		{
                //			promise.set_value (false);
                //		}
                //		catch (std::future_error &)
                //		{
                //		}
            }
        } else {
            //	node->logger->debug (nano::log::type::frontier_req_client, "Error while receiving frontier: {}", ec.message ());
        }
    }
}
