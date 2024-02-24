use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use primitive_types::U256;
use rsnano_core::{Account, BlockHash};
use rsnano_ledger::Ledger;
use rsnano_messages::{FrontierReq, Message};
use tracing::debug;

use crate::transport::{BufferDropPolicy, TrafficType};

use super::BootstrapClient;

pub struct FrontierReqClient {
    data: Mutex<FrontierReqClientData>,
    connection: Arc<BootstrapClient>,
    ledger: Arc<Ledger>,
}

struct FrontierReqClientData {
    current: Account,
    frontier: BlockHash,
    frontiers_age: u32,
    count_limit: u32,
    accounts: VecDeque<(Account, BlockHash)>,
}

impl FrontierReqClient {
    pub fn new(connection: Arc<BootstrapClient>, ledger: Arc<Ledger>) -> Self {
        Self {
            connection,
            ledger,
            data: Mutex::new(FrontierReqClientData {
                current: Account::zero(),
                frontier: BlockHash::zero(),
                frontiers_age: u32::MAX,
                count_limit: u32::MAX,
                accounts: Default::default(),
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
}
