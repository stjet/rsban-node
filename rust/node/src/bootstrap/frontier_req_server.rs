use std::{collections::VecDeque, sync::Arc};

use rsnano_core::{utils::Logger, Account, BlockHash};
use rsnano_ledger::Ledger;

use crate::{
    config::NodeConfig,
    messages::FrontierReq,
    transport::{TcpServer, TcpServerExt},
    utils::{ErrorCode, ThreadPool},
};

pub struct FrontierReqServer {
    connection: Arc<TcpServer>,
    current: Account,
    frontier: BlockHash,
    request: FrontierReq,
    count: usize,
    accounts: VecDeque<(Account, BlockHash)>,
    logger: Arc<dyn Logger>,
    config: NodeConfig,
    thread_pool: Arc<dyn ThreadPool>,
    ledger: Arc<Ledger>,
}

impl FrontierReqServer {
    pub fn new(
        connection: Arc<TcpServer>,
        request: FrontierReq,
        thread_pool: Arc<dyn ThreadPool>,
        logger: Arc<dyn Logger>,
        config: NodeConfig,
        ledger: Arc<Ledger>,
    ) -> Self {
        Self {
            connection,
            current: (request.start.number().overflowing_sub(1.into()).0).into(), // todo: figure out what underflow does
            frontier: BlockHash::zero(),
            request,
            count: 0,
            accounts: VecDeque::new(),
            thread_pool,
            logger,
            config,
            ledger,
        }
    }

    pub fn no_block_sent(&self, ec: ErrorCode, _size: usize) {
        if !ec.is_ok() {
            self.connection.start();
        } else {
            if self.config.logging.network_logging_value {
                self.logger
                    .try_log(&format!("Error sending frontier finish: {:?}", ec));
            }
        }
    }
    pub fn send_confirmed(&self) -> bool {
        self.request.is_confirmed_present()
    }
}
