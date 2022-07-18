mod bootstrap_attempt;
mod bootstrap_initiator;
mod bootstrap_server;
mod channel_tcp_wrapper;

pub(crate) use bootstrap_attempt::*;
pub(crate) use bootstrap_initiator::*;
pub use bootstrap_server::{
    BootstrapRequestsLock, BootstrapServer, BootstrapServerExt, BootstrapServerObserver,
    RequestResponseVisitorFactory,
};

pub use channel_tcp_wrapper::ChannelTcpWrapper;

mod bootstrap_limits {
    pub(crate) const PULL_COUNT_PER_CHECK: u64 = 8 * 1024;
}

#[derive(Clone, Copy, FromPrimitive)]
pub(crate) enum BootstrapMode {
    Legacy,
    Lazy,
    WalletLazy,
}
