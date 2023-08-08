use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use rsnano_core::{utils::Logger, Account, BlockHash, PendingInfo, PendingKey};
use rsnano_ledger::Ledger;

use crate::{
    config::NodeConfig,
    messages::{BulkPullAccount, BulkPullAccountFlags},
    transport::{Socket, TcpServer, TrafficType},
};

struct BulkPullAccountServerImpl {
    connection: Arc<TcpServer>,
    request: BulkPullAccount,
    logger: Arc<dyn Logger>,
    config: NodeConfig,
    ledger: Arc<Ledger>,
    deduplication: HashSet<Account>,
    current_key: PendingKey,
    pending_address_only: bool,
    pending_include_address: bool,
    invalid_request: bool,
}

impl BulkPullAccountServerImpl {
    /**
     * Bulk pull blocks related to an account
     */
    fn set_params(&mut self) {
        // Parse the flags
        self.invalid_request = false;
        self.pending_include_address = false;
        self.pending_address_only = false;
        if self.request.flags == BulkPullAccountFlags::PendingAddressOnly {
            self.pending_address_only = true;
        } else if self.request.flags == BulkPullAccountFlags::PendingHashAmountAndAddress {
            // This is the same as "pending_hash_and_amount" but with the
            // sending address appended, for UI purposes mainly.
            self.pending_include_address = true;
        } else if self.request.flags == BulkPullAccountFlags::PendingHashAndAmount {
            // The defaults are set above
        } else {
            if self.config.logging.bulk_pull_logging_value {
                self.logger.try_log(&format!(
                    "Invalid bulk_pull_account flags supplied {:?}",
                    self.request.flags
                ));
            }

            self.invalid_request = true;

            return;
        }

        /*
         * Initialize the current item from the requested account
         */
        self.current_key.account = self.request.account;
        self.current_key.hash = BlockHash::zero();
    }

    fn send_frontier(&self, server: Arc<Mutex<BulkPullAccountServerImpl>>) {
        /*
         * This function is really the entry point into this class,
         * so handle the invalid_request case by terminating the
         * request without any response
         */
        if !self.invalid_request {
            let stream_transaction = self.ledger.read_txn();

            // Get account balance and frontier block hash
            let account_frontier_hash = self
                .ledger
                .latest(&stream_transaction, &self.request.account)
                .unwrap_or_default();
            let account_frontier_balance =
                self.ledger
                    .account_balance(&stream_transaction, &self.request.account, false);

            // Write the frontier block hash and balance into a buffer
            let mut send_buffer = Vec::new();
            {
                send_buffer.extend_from_slice(account_frontier_hash.as_bytes());
                send_buffer.extend_from_slice(&account_frontier_balance.to_be_bytes());
            }

            // Send the buffer to the requestor
            self.connection.socket.async_write(
                &Arc::new(send_buffer),
                Some(Box::new(move |ec, size| {
                    server.lock().unwrap().sent_action(ec, size);
                })),
                TrafficType::Generic,
            );
        }
    }

    fn send_next_block(&self, server: Arc<Mutex<BulkPullAccountServerImpl>>) {
        /*
         * Get the next item from the queue, it is a tuple with the key (which
         * contains the account and hash) and data (which contains the amount)
         */
        if let Some((block_info_key, block_info)) = self.get_next() {
            /*
             * If we have a new item, emit it to the socket
             */

            let mut send_buffer = Vec::new();
            if self.pending_address_only {
                if self.config.logging.bulk_pull_logging_value {
                    self.logger.try_log(&format!(
                        "Sending address: {}",
                        block_info.source.encode_account()
                    ));
                }
                send_buffer.extend_from_slice(block_info.source.as_bytes());
            } else {
                if self.config.logging.bulk_pull_logging_value {
                    self.logger
                        .try_log(&format!("Sending block: {}", block_info_key.hash));
                }

                send_buffer.extend_from_slice(block_info_key.hash.as_bytes());
                send_buffer.extend_from_slice(&block_info.amount.to_be_bytes());

                if self.pending_include_address {
                    /**
                     ** Write the source address as well, if requested
                     **/
                    send_buffer.extend_from_slice(block_info.source.as_bytes());
                }
            }

            self.connection.socket.async_write(
                &Arc::new(send_buffer),
                Some(Box::new(move |ec, len| {
                    server.lock().unwrap().sent_action(ec, len)
                })),
                TrafficType::Generic,
            );
        } else {
            /*
             * Otherwise, finalize the connection
             */
            if self.config.logging.bulk_pull_logging_value {
                self.logger.try_log("Done sending blocks");
            }

            self.send_finished();
        }
    }

    fn get_next(&mut self) -> Option<(PendingKey, PendingInfo)> {
        loop {
            /*
             * For each iteration of this loop, establish and then
             * destroy a database transaction, to avoid locking the
             * database for a prolonged period.
             */
            let stream_transaction = self.ledger.read_txn();
            let stream = self
                .ledger
                .store
                .pending
                .begin_at_key(&stream_transaction, &self.current_key);

            let Some((key, info)) = stream.current() else {break;};

            /*
             * Get the key for the next value, to use in the next call or iteration
             */
            self.current_key.account = key.account;
            self.current_key.hash = key.hash.number().overflowing_add(1).0.into();

            /*
             * Finish up if the response is for a different account
             */
            if key.account != self.request.account {
                break;
            }

            /*
             * Skip entries where the amount is less than the requested
             * minimum
             */
            if info.amount < self.request.minimum_amount {
                continue;
            }

            /*
             * If the pending_address_only flag is set, de-duplicate the
             * responses.  The responses are the address of the sender,
             * so they are are part of the pending table's information
             * and not key, so we have to de-duplicate them manually.
             */
            if self.pending_address_only {
                if !self.deduplication.insert(info.source) {
                    /*
                     * If the deduplication map gets too
                     * large, clear it out.  This may
                     * result in some duplicates getting
                     * sent to the client, but we do not
                     * want to commit too much memory
                     */
                    if self.deduplication.len() > 4096 {
                        self.deduplication.clear();
                    }
                    continue;
                }
            }

            return Some((*key, *info));
        }

        None
    }
}

pub struct BulkPullAccountServer {
    server: Arc<Mutex<BulkPullAccountServerImpl>>,
}

impl BulkPullAccountServer {
    pub fn new(
        connection: Arc<TcpServer>,
        request: BulkPullAccount,
        logger: Arc<dyn Logger>,
        config: NodeConfig,
        ledger: Arc<Ledger>,
    ) -> Self {
        let server = BulkPullAccountServerImpl {
            connection,
            request,
            logger,
            config,
            ledger,
            deduplication: HashSet::new(),
            current_key: PendingKey::new(Account::zero(), BlockHash::zero()),
            pending_address_only: false,
            pending_include_address: false,
            invalid_request: false,
        };
        Self {
            server: Arc::new(Mutex::new(server)),
        }
    }
}
