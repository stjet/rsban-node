use crate::{
    block_processing::{BlockProcessor, UncheckedMap},
    bootstrap::BootstrapServer,
    cementation::ConfirmingSet,
    config::{NodeConfig, NodeFlags},
    consensus::{LocalVoteHistory, RepTiers, VoteCache, VoteProcessorQueue},
    node_id_key_file::NodeIdKeyFile,
    representatives::RepresentativeRegister,
    stats::{LedgerStats, Stats},
    transport::{
        NetworkFilter, OutboundBandwidthLimiter, SocketObserver, SynCookies, TcpChannels,
        TcpChannelsOptions, TcpMessageManager,
    },
    utils::{
        AsyncRuntime, LongRunningTransactionLogger, ThreadPool, ThreadPoolImpl, TxnTrackingConfig,
    },
    wallets::{Wallets, WalletsExt},
    work::DistributedWorkFactory,
    NetworkParams, OnlineReps, OnlineWeightSampler, TelementryConfig, Telemetry,
};
use rsnano_core::{work::WorkPoolImpl, KeyPair};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::{
    EnvOptions, EnvironmentWrapper, LmdbConfig, LmdbEnv, LmdbStore, NullTransactionTracker,
    SyncStrategy, TransactionTracker,
};
use std::{
    borrow::Borrow,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
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
    pub telemetry: Arc<Telemetry>,
    pub bootstrap_server: Arc<BootstrapServer>,
    pub online_reps: Arc<Mutex<OnlineReps>>,
    pub online_reps_sampler: Arc<OnlineWeightSampler>,
    pub representative_register: Arc<Mutex<RepresentativeRegister>>,
    pub rep_tiers: Arc<RepTiers>,
    pub vote_processor_queue: Arc<VoteProcessorQueue>,
    pub history: Arc<LocalVoteHistory>,
    pub confirming_set: Arc<ConfirmingSet>,
    pub vote_cache: Arc<Mutex<VoteCache>>,
    pub block_processor: Arc<BlockProcessor>,
    pub wallets: Arc<Wallets>,
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

        let telemetry_config = TelementryConfig {
            enable_ongoing_requests: !flags.disable_ongoing_telemetry_requests,
            enable_ongoing_broadcasts: !flags.disable_providing_telemetry_metrics,
        };

        let unchecked = Arc::new(UncheckedMap::new(
            config.max_unchecked_blocks as usize,
            Arc::clone(&stats),
            flags.disable_block_processor_unchecked_deletion,
        ));

        let telemetry = Arc::new(Telemetry::new(
            telemetry_config,
            config.clone(),
            Arc::clone(&stats),
            Arc::clone(&ledger),
            Arc::clone(&unchecked),
            network_params.clone(),
            Arc::clone(&channels),
            node_id.clone(),
        ));

        let bootstrap_server = Arc::new(BootstrapServer::new(
            Arc::clone(&stats),
            Arc::clone(&ledger),
        ));

        let mut online_reps = OnlineReps::new(Arc::clone(&ledger));
        online_reps.set_weight_period(Duration::from_secs(network_params.node.weight_period));
        online_reps.set_online_weight_minimum(config.online_weight_minimum);

        let mut online_reps_sampler = OnlineWeightSampler::new(Arc::clone(&ledger));
        online_reps_sampler.set_online_weight_minimum(config.online_weight_minimum);
        online_reps_sampler.set_max_samples(network_params.node.max_weight_samples);
        let online_reps_sampler = Arc::new(online_reps_sampler);
        online_reps.set_trended(online_reps_sampler.calculate_trend());
        let online_reps = Arc::new(Mutex::new(online_reps));

        let representative_register = Arc::new(Mutex::new(RepresentativeRegister::new(
            Arc::clone(&ledger),
            Arc::clone(&online_reps),
            Arc::clone(&stats),
            network_params.network.protocol_info(),
        )));

        let rep_tiers = Arc::new(RepTiers::new(
            Arc::clone(&ledger),
            network_params.clone(),
            Arc::clone(&online_reps),
            Arc::clone(&stats),
        ));

        let vote_processor_queue = Arc::new(VoteProcessorQueue::new(
            flags.vote_processor_capacity,
            Arc::clone(&stats),
            Arc::clone(&online_reps),
            Arc::clone(&ledger),
            Arc::clone(&rep_tiers),
        ));

        let history = Arc::new(LocalVoteHistory::new(network_params.voting.max_cache));

        let confirming_set = Arc::new(ConfirmingSet::new(
            Arc::clone(&ledger),
            config.confirming_set_batch_time,
        ));

        let vote_cache = Arc::new(Mutex::new(VoteCache::new(
            config.vote_cache.clone(),
            Arc::clone(&stats),
        )));

        let block_processor = Arc::new(BlockProcessor::new(
            Arc::new(config.clone()),
            Arc::new(flags.clone()),
            Arc::clone(&ledger),
            Arc::clone(&unchecked),
            Arc::clone(&stats),
            Arc::new(network_params.work.clone()),
        ));

        let distributed_work = Arc::new(DistributedWorkFactory::new(
            Arc::clone(&work),
            Arc::clone(&async_rt),
        ));

        let mut wallets_path = application_path.clone();
        wallets_path.push("wallets.ldb");

        let mut wallets_lmdb_config = config.lmdb_config.clone();
        wallets_lmdb_config.sync = SyncStrategy::Always;
        wallets_lmdb_config.map_size = 1024 * 1024 * 1024;
        let wallets_options = EnvOptions {
            config: wallets_lmdb_config,
            use_no_mem_init: false,
        };
        let wallets_env = Arc::new(LmdbEnv::with_options(wallets_path, &wallets_options).unwrap());

        let wallets = Arc::new(
            Wallets::new(
                config.enable_voting,
                wallets_env,
                Arc::clone(&ledger),
                &config,
                network_params.kdf_work,
                network_params.work.clone(),
                Arc::clone(&distributed_work),
                network_params.clone(),
                Arc::clone(&workers),
                Arc::clone(&block_processor),
                Arc::clone(&online_reps),
                Arc::clone(&channels),
                Arc::clone(&confirming_set),
            )
            .expect("Could not create wallet"),
        );
        wallets.initialize2();

        Self {
            node_id,
            workers,
            bootstrap_workers: Arc::new(ThreadPoolImpl::create(
                config.bootstrap_serving_threads as usize,
                "Bootstrap work".to_string(),
            )),
            distributed_work,
            unchecked,
            telemetry,
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
            bootstrap_server,
            online_reps,
            online_reps_sampler,
            representative_register,
            rep_tiers,
            vote_processor_queue,
            history,
            confirming_set,
            vote_cache,
            block_processor,
            wallets,
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
