use std::sync::Arc;

use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, BlockType};
use rsnano_ledger::Ledger;

use crate::{
    messages::BulkPull,
    transport::{Socket, TcpServer, TcpServerExt},
    utils::ThreadPool,
};

pub struct BulkPullServer {
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    enable_logging: bool,
    pub sent_count: u32,
    pub max_count: u32,
    pub include_start: bool,
    pub current: BlockHash,
    pub request: BulkPull,
    connection: Arc<TcpServer>,
    thread_pool: Arc<dyn ThreadPool>,
}

impl BulkPullServer {
    pub fn new(
        request: BulkPull,
        connection: Arc<TcpServer>,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        thread_pool: Arc<dyn ThreadPool>,
        enable_logging: bool,
    ) -> Self {
        Self {
            ledger,
            logger,
            enable_logging,
            thread_pool,
            sent_count: 0,
            max_count: 0,
            include_start: false,
            current: BlockHash::zero(),
            request,
            connection,
        }
    }

    pub fn set_current_end(&mut self) {
        self.include_start = false;
        let transaction = self.ledger.read_txn();
        if !self
            .ledger
            .store
            .block()
            .exists(transaction.txn(), &self.request.end)
        {
            if self.enable_logging {
                self.logger.try_log(&format!(
                    "Bulk pull end block doesn't exist: {}, sending everything",
                    self.request.end
                ));
            }
            self.request.end = BlockHash::zero();
        }

        if self
            .ledger
            .store
            .block()
            .exists(transaction.txn(), &self.request.start.into())
        {
            if self.enable_logging {
                self.logger.try_log(&format!(
                    "Bulk pull request for block hash: {}",
                    self.request.start
                ));
            }

            self.current = if self.ascending() {
                self.ledger
                    .store
                    .block()
                    .successor(transaction.txn(), &self.request.start.into())
                    .unwrap_or_default()
            } else {
                self.request.start.into()
            };
            self.include_start = true;
        } else {
            if let Some(info) = self
                .ledger
                .account_info(transaction.txn(), &self.request.start.into())
            {
                self.current = if self.ascending() {
                    info.open_block
                } else {
                    info.head
                };
                if !self.request.end.is_zero() {
                    let account = self
                        .ledger
                        .account(transaction.txn(), &self.request.end)
                        .unwrap_or_default();
                    if account != self.request.start.into() {
                        if self.enable_logging {
                            self.logger.try_log(&format!(
                                "Request for block that is not on account chain: {} not on {}",
                                self.request.end,
                                Account::from(self.request.start).encode_account()
                            ));
                        }
                        self.current = self.request.end;
                    }
                }
            } else {
                if self.enable_logging {
                    self.logger.try_log(&format!(
                        "Request for unknown account: {}",
                        Account::from(self.request.start).encode_account()
                    ));
                }
                self.current = self.request.end;
            }
        }

        self.sent_count = 0;
        if self.request.is_count_present() {
            self.max_count = self.request.count;
        } else {
            self.max_count = 0;
        }
    }

    fn ascending(&self) -> bool {
        self.request.is_ascending()
    }

    pub fn send_finished(&self) {
        let send_buffer = Arc::new(vec![BlockType::NotABlock as u8]);
        if self.enable_logging {
            self.logger.try_log("Bulk sending finished");
        }

        let enable_logging = self.enable_logging;
        let logger = self.logger.clone();
        let connection = self.connection.clone();

        self.connection.socket.async_write(
            &send_buffer,
            Some(Box::new(move |ec, _| {
                if ec.is_ok() {
                    connection.start();
                } else {
                    if enable_logging {
                        logger.try_log("Unable to send not-a-block");
                    }
                }
            })),
            crate::transport::TrafficType::Generic,
        )
    }

    pub fn get_next(&mut self) -> Option<BlockEnum> {
        let mut send_current = false;
        let mut set_current_to_end = false;

        /*
         * Determine if we should reply with a block
         *
         * If our cursor is on the final block, we should signal that we
         * are done by returning a null result.
         *
         * Unless we are including the "start" member and this is the
         * start member, then include it anyway.
         */
        if self.current != self.request.end {
            send_current = true;
        } else if self.current == self.request.end && self.include_start {
            send_current = true;

            /*
             * We also need to ensure that the next time
             * are invoked that we return a null result
             */
            set_current_to_end = true;
        }

        /*
         * Account for how many blocks we have provided.  If this
         * exceeds the requested maximum, return an empty object
         * to signal the end of results
         */
        if self.max_count != 0 && self.sent_count >= self.max_count {
            send_current = false;
        }

        let mut result = None;
        if send_current {
            {
                let txn = self.ledger.read_txn();
                result = self.ledger.get_block(txn.txn(), &self.current);
            }

            if let Some(result) = &result {
                if !set_current_to_end {
                    let next = if self.ascending() {
                        result.successor().unwrap_or_default()
                    } else {
                        result.previous()
                    };
                    if !next.is_zero() {
                        self.current = next;
                    } else {
                        self.current = self.request.end;
                    }
                } else {
                    self.current = self.request.end;
                }
            } else {
                self.current = self.request.end;
            }

            self.sent_count += 1;
        }

        /*
         * Once we have processed "get_next()" once our cursor is no longer on
         * the "start" member, so this flag is not relevant is always false.
         */
        self.include_start = false;

        result
    }
}
