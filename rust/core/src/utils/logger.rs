pub trait Logger: Send + Sync {
    fn log(&self, level: LogLevel, tag: LogType, message: &str);
    fn debug(&self, tag: LogType, message: &str);
    fn info(&self, tag: LogType, message: &str);
    fn warn(&self, tag: LogType, message: &str);
    fn error(&self, tag: LogType, message: &str);
    fn critical(&self, tag: LogType, message: &str);
}
pub struct NullLogger {}

impl NullLogger {
    pub fn new() -> Self {
        Self {}
    }
}

impl Logger for NullLogger {
    fn log(&self, _level: LogLevel, _tag: LogType, _message: &str) {}
    fn debug(&self, _tag: LogType, _message: &str) {}
    fn info(&self, _tag: LogType, _message: &str) {}
    fn warn(&self, _tag: LogType, _message: &str) {}
    fn error(&self, _tag: LogType, _message: &str) {}
    fn critical(&self, _tag: LogType, _message: &str) {}
}

pub struct ConsoleLogger {}

impl ConsoleLogger {
    pub fn new() -> Self {
        Self {}
    }
}

impl Logger for ConsoleLogger {
    fn log(&self, level: LogLevel, tag: LogType, message: &str) {
        println!("{level:?} {tag:?} {message}");
    }

    fn debug(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Debug, tag, message);
    }

    fn info(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Info, tag, message);
    }

    fn warn(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Warn, tag, message);
    }

    fn error(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Error, tag, message);
    }

    fn critical(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Critical, tag, message);
    }
}

#[derive(FromPrimitive, Debug)]
#[repr(u8)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
    Off,
}

#[derive(FromPrimitive, Debug)]
#[repr(u8)]
pub enum LogType {
    All = 0, // reserved

    Generic,
    Init,
    Config,
    Logging,
    Node,
    NodeWrapper,
    Daemon,
    DaemonRpc,
    DaemonWallet,
    Wallet,
    Qt,
    Rpc,
    RpcConnection,
    RpcCallbacks,
    RpcRequest,
    Ipc,
    IpcServer,
    Websocket,
    Tls,
    ActiveTransactions,
    Election,
    Blockprocessor,
    Network,
    Channel,
    Socket,
    SocketServer,
    Tcp,
    TcpServer,
    TcpListener,
    Prunning,
    ConfProcessorBounded,
    ConfProcessorUnbounded,
    DistributedWork,
    EpochUpgrader,
    OpenclWork,
    Upnp,
    Repcrawler,
    Lmdb,
    Rocksdb,
    TxnTracker,
    GapCache,
    VoteProcessor,
    BulkPullClient,
    BulkPullServer,
    BulkPullAccountClient,
    BulkPullAccountServer,
    BulkPushClient,
    BulkPushServer,
    FrontierReqClient,
    FrontierReqServer,
    Bootstrap,
    BootstrapLazy,
    BootstrapLegacy,
}

#[derive(FromPrimitive)]
#[repr(u8)]
pub enum LogDetail {
    All = 0, // reserved

    // node
    ProcessConfirmed,

    // active_transactions
    ActiveStarted,
    ActiveStopped,

    // election
    ElectionConfirmed,
    ElectionExpired,

    // blockprocessor
    BlockProcessed,

    // vote_processor
    VoteProcessed,

    // network
    MessageReceived,
    MessageSent,
    MessageDropped,

    // bulk pull/push
    PulledBlock,
    SendingBlock,
    SendingPending,
    SendingFrontier,
    RequestingAccountOrHead,
    RequestingPending,
}
