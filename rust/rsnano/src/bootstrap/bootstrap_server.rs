use std::sync::Arc;

use crate::{logger_mt::Logger, transport::SocketImpl, NodeConfig};

pub struct BootstrapServer {
    socket: Arc<SocketImpl>,
    config: Arc<NodeConfig>,
    logger: Arc<dyn Logger>,
}

impl BootstrapServer {
    pub fn new(socket: Arc<SocketImpl>, config: Arc<NodeConfig>, logger: Arc<dyn Logger>) -> Self {
        Self {
            socket,
            config,
            logger,
        }
    }
}

impl Drop for BootstrapServer {
    fn drop(&mut self) {
        if self.config.logging.bulk_pull_logging_value {
            self.logger.try_log("Exiting incoming TCP/bootstrap server");
        }
    }
}
