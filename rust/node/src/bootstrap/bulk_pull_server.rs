use std::sync::Arc;

use rsnano_core::{utils::Logger, Account, BlockHash};
use rsnano_ledger::Ledger;

use crate::messages::BulkPull;

pub struct BulkPullServer {
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    enable_logging: bool,
    pub sent_count: u32,
    pub max_count: u32,
    pub include_start: bool,
    pub current: BlockHash,
    pub request: BulkPull,
}

impl BulkPullServer {
    pub fn new(
        request: BulkPull,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        enable_logging: bool,
    ) -> Self {
        Self {
            ledger,
            logger,
            enable_logging,
            sent_count: 0,
            max_count: 0,
            include_start: false,
            current: BlockHash::zero(),
            request,
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
}
