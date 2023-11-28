use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
};

use rsnano_core::{
    utils::{seconds_since_epoch, Logger},
    Account, BlockHash,
};
use rsnano_ledger::Ledger;
use rsnano_messages::FrontierReq;

use crate::{
    transport::{SocketExtensions, TcpServer, TcpServerExt, TrafficType},
    utils::{ErrorCode, ThreadPool},
};

/// Server side of a frontier request. Created when a tcp_server receives a frontier_req message and exited when end-of-list is reached.
pub struct FrontierReqServer {
    server_impl: Arc<Mutex<FrontierReqServerImpl>>,
}

impl FrontierReqServer {
    pub fn new(
        connection: Arc<TcpServer>,
        request: FrontierReq,
        thread_pool: Arc<dyn ThreadPool>,
        logger: Arc<dyn Logger>,
        enable_logging: bool,
        enable_network_logging: bool,
        ledger: Arc<Ledger>,
    ) -> Self {
        let result = Self {
            server_impl: Arc::new(Mutex::new(FrontierReqServerImpl {
                connection,
                current: (request.start.number().overflowing_sub(1.into()).0).into(), // todo: figure out what underflow does
                frontier: BlockHash::zero(),
                request,
                count: 0,
                accounts: VecDeque::new(),
                thread_pool: Arc::downgrade(&thread_pool),
                logger,
                enable_logging,
                enable_network_logging,
                ledger,
            })),
        };
        result.server_impl.lock().unwrap().next();
        result
    }

    pub fn send_next(&self) {
        let server_clone = Arc::clone(&self.server_impl);
        self.server_impl.lock().unwrap().send_next(server_clone);
    }

    pub fn current(&self) -> Account {
        self.server_impl.lock().unwrap().current
    }

    pub fn frontier(&self) -> BlockHash {
        self.server_impl.lock().unwrap().frontier
    }
}

struct FrontierReqServerImpl {
    connection: Arc<TcpServer>,
    current: Account,
    frontier: BlockHash,
    request: FrontierReq,
    count: usize,
    accounts: VecDeque<(Account, BlockHash)>,
    logger: Arc<dyn Logger>,
    enable_logging: bool,
    enable_network_logging: bool,
    thread_pool: Weak<dyn ThreadPool>,
    ledger: Arc<Ledger>,
}

impl FrontierReqServerImpl {
    pub fn no_block_sent(&self, ec: ErrorCode, _size: usize) {
        if ec.is_ok() {
            self.connection.start();
        } else {
            if self.enable_network_logging {
                self.logger
                    .try_log(&format!("Error sending frontier finish: {:?}", ec));
            }
        }
    }

    pub fn send_confirmed(&self) -> bool {
        self.request.only_confirmed
    }

    pub fn send_finished(&self, server: Arc<Mutex<FrontierReqServerImpl>>) {
        let mut send_buffer = Vec::with_capacity(64);
        send_buffer.extend_from_slice(Account::zero().as_bytes());
        send_buffer.extend_from_slice(BlockHash::zero().as_bytes());
        if self.enable_network_logging {
            self.logger.try_log("Frontier sending finished");
        }

        self.connection.socket.async_write(
            &Arc::new(send_buffer),
            Some(Box::new(move |ec, size| {
                server.lock().unwrap().no_block_sent(ec, size);
            })),
            TrafficType::Generic,
        )
    }

    pub fn send_next(&mut self, server: Arc<Mutex<FrontierReqServerImpl>>) {
        if !self.current.is_zero() && self.count < self.request.count as usize {
            let mut send_buffer = Vec::with_capacity(64);
            send_buffer.extend_from_slice(self.current.as_bytes());
            send_buffer.extend_from_slice(self.frontier.as_bytes());
            debug_assert!(!self.current.is_zero());
            debug_assert!(!self.frontier.is_zero());
            if self.enable_logging {
                self.logger.try_log(&format!(
                    "Sending frontier for {} {}",
                    self.current.encode_account(),
                    self.frontier
                ));
            }
            self.next();
            self.connection.socket.async_write(
                &Arc::new(send_buffer),
                Some(Box::new(move |ec, size| {
                    let server_clone = Arc::clone(&server);
                    server.lock().unwrap().sent_action(ec, size, server_clone);
                })),
                TrafficType::Generic,
            );
        } else {
            self.send_finished(server);
        }
    }

    pub fn next(&mut self) {
        // Filling accounts deque to prevent often read transactions
        if self.accounts.is_empty() {
            let now = seconds_since_epoch();
            let disable_age_filter = self.request.age == u32::MAX;
            let max_size = 128;
            let transaction = self.ledger.read_txn();
            if !self.send_confirmed() {
                let mut i = self.ledger.store.account.begin_account(
                    &transaction,
                    &self.current.number().overflowing_add(1.into()).0.into(),
                );
                while let Some((account, info)) = i.current() {
                    if self.accounts.len() >= max_size {
                        break;
                    }
                    if disable_age_filter || (now - info.modified) <= self.request.age as u64 {
                        self.accounts.push_back((*account, info.head))
                    }
                    i.next();
                }
            } else {
                let mut i = self.ledger.store.confirmation_height.begin_at_account(
                    &transaction,
                    &self.current.number().overflowing_add(1.into()).0.into(),
                );
                while let Some((account, info)) = i.current() {
                    if self.accounts.len() >= max_size {
                        break;
                    }

                    let confirmed_frontier = info.frontier;
                    if !confirmed_frontier.is_zero() {
                        self.accounts.push_back((*account, confirmed_frontier));
                    }

                    i.next();
                }
            }

            /* If loop breaks before max_size, then accounts_end () is reached. Add empty record to finish frontier_req_server */
            if self.accounts.len() != max_size {
                self.accounts
                    .push_back((Account::zero(), BlockHash::zero()));
            }
        }

        // Retrieving accounts from deque
        if let Some((account, frontier)) = self.accounts.pop_front() {
            self.current = account;
            self.frontier = frontier;
        }
    }

    pub fn sent_action(
        &mut self,
        ec: ErrorCode,
        _size: usize,
        server: Arc<Mutex<FrontierReqServerImpl>>,
    ) {
        let Some(thread_pool) = self.thread_pool.upgrade() else {
            return;
        };

        if ec.is_ok() {
            self.count += 1;
            thread_pool.push_task(Box::new(move || {
                let server_clone = Arc::clone(&server);
                server.lock().unwrap().send_next(server_clone);
            }));
        } else {
            if self.enable_network_logging {
                self.logger
                    .try_log(&format!("Error sending frontier pair: {:?}", ec));
            }
        }
    }
}
