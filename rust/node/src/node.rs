use crate::{
    block_processing::UncheckedMap,
    config::{NodeConfig, NodeFlags},
    node_id_key_file::NodeIdKeyFile,
    stats::{LedgerStats, Stats},
    transport::{
        NetworkFilter, OutboundBandwidthLimiter, SocketObserver, SynCookies, TcpChannels,
        TcpChannelsOptions, TcpMessageManager,
    },
    utils::{
        AsyncRuntime, LongRunningTransactionLogger, ThreadPool, ThreadPoolImpl, TxnTrackingConfig,
    },
    work::DistributedWorkFactory,
    NetworkParams,
};
use rsnano_core::{work::WorkPoolImpl, KeyPair};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::{
    EnvOptions, EnvironmentWrapper, LmdbConfig, LmdbStore, NullTransactionTracker,
    TransactionTracker,
};
use std::{
    borrow::Borrow,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub struct Node {
    pub async_rt: Arc<AsyncRuntime>,
    application_path: PathBuf,
    pub node_id: KeyPair,
    pub config: NodeConfig,
    network_params: NetworkParams,
    pub stats: Arc<Stats>,
    pub workers: Arc<dyn ThreadPool>,
    pub bootstrap_workers: Arc<dyn ThreadPool>,
    flags: NodeFlags,
    work: Arc<WorkPoolImpl>,
    pub distributed_work: Arc<DistributedWorkFactory>,
    pub store: Arc<LmdbStore>,
    pub unchecked: Arc<UncheckedMap>,
    pub ledger: Arc<Ledger>,
    pub outbound_limiter: Arc<OutboundBandwidthLimiter>,
    pub syn_cookies: Arc<SynCookies>,
    pub channels: Arc<TcpChannels>,
}

impl Node {
    pub fn new(
        async_rt: Arc<AsyncRuntime>,
        application_path: impl Into<PathBuf>,
        config: NodeConfig,
        network_params: NetworkParams,
        flags: NodeFlags,
        work: Arc<WorkPoolImpl>,
        socket_observer: Arc<dyn SocketObserver>,
    ) -> Self {
        let application_path = application_path.into();
        let node_id = NodeIdKeyFile::default()
            .initialize(&application_path)
            .unwrap();

        let stats = Arc::new(Stats::new(config.stat_config.clone()));

        let store = make_store(
            &application_path,
            true,
            &config.diagnostics_config.txn_tracking,
            Duration::from_millis(config.block_processor_batch_max_time_ms as u64),
            config.lmdb_config.clone(),
            config.backup_before_upgrade,
        )
        .expect("Could not create LMDB store");

        let mut ledger = Ledger::new(
            Arc::clone(&store),
            network_params.ledger.clone(),
            config.representative_vote_weight_minimum,
        )
        .expect("Could not initialize ledger");

        ledger.set_observer(Arc::new(LedgerStats::new(Arc::clone(&stats))));
        let ledger = Arc::new(ledger);

        let outbound_limiter = Arc::new(OutboundBandwidthLimiter::new(config.borrow().into()));
        let syn_cookies = Arc::new(SynCookies::new(network_params.network.max_peers_per_ip));
        let workers: Arc<dyn ThreadPool> = Arc::new(ThreadPoolImpl::create(
            config.background_threads as usize,
            "Worker".to_string(),
        ));

        let channels = Arc::new(TcpChannels::new(TcpChannelsOptions {
            node_config: config.clone(),
            publish_filter: Arc::new(NetworkFilter::new(256 * 1024)),
            async_rt: Arc::clone(&async_rt),
            network: network_params.clone(),
            stats: Arc::clone(&stats),
            tcp_message_manager: Arc::new(TcpMessageManager::new(
                config.tcp_incoming_connections_max as usize,
            )),
            port: config.peering_port.unwrap_or(0),
            flags: flags.clone(),
            limiter: Arc::clone(&outbound_limiter),
            node_id: node_id.clone(),
            syn_cookies: Arc::clone(&syn_cookies),
            workers: Arc::clone(&workers),
            observer: socket_observer,
        }));

        Self {
            node_id,
            workers,
            bootstrap_workers: Arc::new(ThreadPoolImpl::create(
                config.bootstrap_serving_threads as usize,
                "Bootstrap work".to_string(),
            )),
            distributed_work: Arc::new(DistributedWorkFactory::new(
                Arc::clone(&work),
                Arc::clone(&async_rt),
            )),
            unchecked: Arc::new(UncheckedMap::new(
                config.max_unchecked_blocks as usize,
                Arc::clone(&stats),
                flags.disable_block_processor_unchecked_deletion,
            )),
            outbound_limiter,
            syn_cookies,
            channels,
            ledger,
            store,
            stats,
            application_path,
            network_params,
            config,
            flags,
            work,
            async_rt,
        }
    }
}

fn make_store(
    path: &Path,
    add_db_postfix: bool,
    txn_tracking_config: &TxnTrackingConfig,
    block_processor_batch_max_time: Duration,
    lmdb_config: LmdbConfig,
    backup_before_upgrade: bool,
) -> anyhow::Result<Arc<LmdbStore>> {
    let mut path = PathBuf::from(path);
    if add_db_postfix {
        path.push("data.ldb");
    }

    let txn_tracker: Arc<dyn TransactionTracker> = if txn_tracking_config.enable {
        Arc::new(LongRunningTransactionLogger::new(
            txn_tracking_config.clone(),
            block_processor_batch_max_time,
        ))
    } else {
        Arc::new(NullTransactionTracker::new())
    };

    let options = EnvOptions {
        config: lmdb_config,
        use_no_mem_init: true,
    };

    let store = LmdbStore::<EnvironmentWrapper>::open(&path)
        .options(&options)
        .backup_before_upgrade(backup_before_upgrade)
        .txn_tracker(txn_tracker)
        .build()?;
    Ok(Arc::new(store))
}
