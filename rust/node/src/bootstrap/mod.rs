mod bootstrap_attempt;
mod bootstrap_attempts;
mod bootstrap_client;
mod bootstrap_initiator;
mod bootstrap_lazy;
mod bulk_pull_account_server;
mod bulk_pull_server;
mod channel_tcp_wrapper;
mod frontier_req_server;
mod pulls_cache;

pub use bootstrap_attempt::*;
pub use bootstrap_initiator::*;
pub use frontier_req_server::FrontierReqServer;

pub use bootstrap_client::{
    BootstrapClient, BootstrapClientObserver, BootstrapClientObserverWeakPtr,
};

pub use bootstrap_attempts::BootstrapAttempts;
pub use bootstrap_lazy::BootstrapAttemptLazy;
pub use bulk_pull_server::BulkPullServer;
pub use channel_tcp_wrapper::ChannelTcpWrapper;
pub use pulls_cache::{PullInfo, PullsCache};

pub mod bootstrap_limits {
    pub const PULL_COUNT_PER_CHECK: u64 = 8 * 1024;
    pub const BOOTSTRAP_MINIMUM_ELAPSED_SECONDS_BLOCKRATE: f64 = 0.02;
}

#[derive(Clone, Copy, FromPrimitive)]
pub enum BootstrapMode {
    Legacy,
    Lazy,
    WalletLazy,
    Ascending,
}

pub enum BootstrapStrategy {
    Lazy(BootstrapAttemptLazy),
    Other(BootstrapAttempt),
}

impl BootstrapStrategy {
    pub fn attempt(&self) -> &BootstrapAttempt {
        match self {
            BootstrapStrategy::Other(i) => i,
            BootstrapStrategy::Lazy(i) => &i.attempt,
        }
    }
}
