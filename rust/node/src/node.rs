use crate::{
    block_processing::{
        BacklogPopulation, BacklogPopulationConfig, BlockProcessor, LocalBlockBroadcaster,
        LocalBlockBroadcasterExt, UncheckedMap,
    },
    bootstrap::{BootstrapAscending, BootstrapInitiator, BootstrapInitiatorExt, BootstrapServer},
    cementation::ConfirmingSet,
    config::{FrontiersConfirmationMode, NodeConfig, NodeFlags},
    consensus::{
        AccountBalanceChangedCallback, ActiveTransactions, ActiveTransactionsExt,
        ElectionEndCallback, HintedScheduler, LocalVoteHistory, ManualScheduler,
        OptimisticScheduler, PriorityScheduler, ProcessLiveDispatcher, RepTiers, RequestAggregator,
        RequestAggregatorExt, VoteCache, VoteGenerator, VoteProcessor, VoteProcessorQueue,
    },
    node_id_key_file::NodeIdKeyFile,
    representatives::{RepCrawler, RepresentativeRegister},
    stats::{LedgerStats, Stats},
    transport::{
        ChannelEnum, InboundCallback, LiveMessageProcessor, NetworkFilter, NetworkThreads,
        OutboundBandwidthLimiter, SocketObserver, SynCookies, TcpChannels, TcpChannelsOptions,
        TcpListener, TcpMessageManager,
    },
    utils::{
        AsyncRuntime, LongRunningTransactionLogger, ThreadPool, ThreadPoolImpl, TxnTrackingConfig,
    },
    wallets::{Wallets, WalletsExt},
    websocket::create_websocket_server,
    work::DistributedWorkFactory,
    NetworkParams, OnlineReps, OnlineWeightSampler, TelementryConfig, Telemetry,
};
use rsnano_core::{work::WorkPoolImpl, KeyPair, Vote, VoteCode};
use rsnano_ledger::Ledger;
use rsnano_messages::DeserializedMessage;
use rsnano_store_lmdb::{
    EnvOptions, EnvironmentWrapper, LmdbConfig, LmdbEnv, LmdbStore, NullTransactionTracker,
    SyncStrategy, TransactionTracker,
};
use std::{
    borrow::Borrow,
    net::{Ipv6Addr, SocketAddrV6},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
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
    pub vote_generator: Arc<VoteGenerator>,
    pub final_generator: Arc<VoteGenerator>,
    pub active: Arc<ActiveTransactions>,
    pub vote_processor: Arc<VoteProcessor>,
    pub websocket: Option<Arc<crate::websocket::WebsocketListener>>,
    pub bootstrap_initiator: Arc<BootstrapInitiator>,
    pub rep_crawler: Arc<RepCrawler>,
    pub tcp_listener: Arc<TcpListener>,
    pub hinted_scheduler: Arc<HintedScheduler>,
    pub manual_scheduler: Arc<ManualScheduler>,
    pub optimistic_scheduler: Arc<OptimisticScheduler>,
    pub priority_scheduler: Arc<PriorityScheduler>,
    pub request_aggregator: Arc<RequestAggregator>,
    pub backlog_population: Arc<BacklogPopulation>,
    pub ascendboot: Arc<BootstrapAscending>,
    pub local_block_broadcaster: Arc<LocalBlockBroadcaster>,
    pub process_live_dispatcher: Arc<ProcessLiveDispatcher>,
    pub live_message_processor: Arc<LiveMessageProcessor>,
    pub network_threads: Arc<NetworkThreads>,
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
        election_end: ElectionEndCallback,
        account_balance_changed: AccountBalanceChangedCallback,
        on_vote: Box<dyn Fn(&Arc<Vote>, &Arc<ChannelEnum>, VoteCode) + Send + Sync>,
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
            observer: Arc::clone(&socket_observer),
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

        let inbound_impl: Arc<
            RwLock<Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Send + Sync>>,
        > = Arc::new(RwLock::new(Box::new(|_msg, _channel| {
            panic!("inbound callback not set");
        })));
        let inbound_impl_clone = Arc::clone(&inbound_impl);
        let inbound: InboundCallback =
            Arc::new(move |msg: DeserializedMessage, channel: Arc<ChannelEnum>| {
                let cb = inbound_impl_clone.read().unwrap();
                (*cb)(msg, channel);
            });

        let vote_generator = Arc::new(VoteGenerator::new(
            Arc::clone(&ledger),
            Arc::clone(&wallets),
            Arc::clone(&history),
            false, //none-final
            Arc::clone(&stats),
            Arc::clone(&representative_register),
            Arc::clone(&channels),
            Arc::clone(&vote_processor_queue),
            network_params.network.clone(),
            Arc::clone(&async_rt),
            node_id.public_key(),
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, channels.port(), 0, 0),
            Arc::clone(&inbound),
            Duration::from_secs(network_params.voting.delay_s as u64),
            Duration::from_millis(config.vote_generator_delay_ms as u64),
            config.vote_generator_threshold as usize,
        ));

        let final_generator = Arc::new(VoteGenerator::new(
            Arc::clone(&ledger),
            Arc::clone(&wallets),
            Arc::clone(&history),
            true, //final
            Arc::clone(&stats),
            Arc::clone(&representative_register),
            Arc::clone(&channels),
            Arc::clone(&vote_processor_queue),
            network_params.network.clone(),
            Arc::clone(&async_rt),
            node_id.public_key(),
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, channels.port(), 0, 0),
            Arc::clone(&inbound),
            Duration::from_secs(network_params.voting.delay_s as u64),
            Duration::from_millis(config.vote_generator_delay_ms as u64),
            config.vote_generator_threshold as usize,
        ));

        let active = Arc::new(ActiveTransactions::new(
            network_params.clone(),
            Arc::clone(&online_reps),
            Arc::clone(&wallets),
            config.clone(),
            Arc::clone(&ledger),
            Arc::clone(&confirming_set),
            Arc::clone(&workers),
            Arc::clone(&history),
            Arc::clone(&block_processor),
            Arc::clone(&vote_generator),
            Arc::clone(&final_generator),
            Arc::clone(&channels),
            Arc::clone(&vote_cache),
            Arc::clone(&stats),
            election_end,
            account_balance_changed,
            Arc::clone(&representative_register),
            flags.clone(),
        ));

        active.initialize();

        let vote_processor = Arc::new(VoteProcessor::new(
            Arc::clone(&vote_processor_queue),
            Arc::clone(&active),
            Arc::clone(&stats),
            on_vote,
        ));

        let websocket = create_websocket_server(
            config.websocket_config.clone(),
            Arc::clone(&wallets),
            Arc::clone(&async_rt),
            &active,
            &telemetry,
            &vote_processor,
        );

        let bootstrap_initiator = Arc::new(BootstrapInitiator::new(
            config.clone(),
            flags.clone(),
            Arc::clone(&channels),
            Arc::clone(&async_rt),
            Arc::clone(&workers),
            network_params.clone(),
            Arc::clone(&socket_observer),
            Arc::clone(&stats),
            Arc::clone(&outbound_limiter),
            Arc::clone(&block_processor),
            websocket.clone(),
            Arc::clone(&ledger),
        ));
        bootstrap_initiator.initialize();
        bootstrap_initiator.start();

        let rep_crawler = Arc::new(RepCrawler::new(
            Arc::clone(&representative_register),
            Arc::clone(&stats),
            config.rep_crawler_query_timeout,
            Arc::clone(&online_reps),
            config.clone(),
            network_params.clone(),
            Arc::clone(&channels),
            Arc::clone(&async_rt),
            Arc::clone(&ledger),
            Arc::clone(&active),
        ));

        let tcp_listener = Arc::new(TcpListener::new(
            channels.port(),
            config.tcp_incoming_connections_max as usize,
            config.clone(),
            Arc::clone(&channels),
            Arc::clone(&syn_cookies),
            network_params.clone(),
            flags.clone(),
            Arc::clone(&async_rt),
            socket_observer,
            Arc::clone(&stats),
            Arc::clone(&workers),
            Arc::clone(&block_processor),
            Arc::clone(&bootstrap_initiator),
            Arc::clone(&ledger),
            Arc::new(node_id.clone()),
        ));

        let hinted_scheduler = Arc::new(HintedScheduler::new(
            config.hinted_scheduler.clone(),
            Arc::clone(&active),
            Arc::clone(&ledger),
            Arc::clone(&stats),
            Arc::clone(&vote_cache),
            Arc::clone(&confirming_set),
            Arc::clone(&online_reps),
        ));

        let manual_scheduler = Arc::new(ManualScheduler::new(
            Arc::clone(&stats),
            Arc::clone(&active),
        ));

        let optimistic_scheduler = Arc::new(OptimisticScheduler::new(
            config.optimistic_scheduler.clone(),
            Arc::clone(&stats),
            Arc::clone(&active),
            network_params.network.clone(),
            Arc::clone(&ledger),
            Arc::clone(&confirming_set),
        ));

        let priority_scheduler = Arc::new(PriorityScheduler::new(
            Arc::clone(&ledger),
            Arc::clone(&stats),
            Arc::clone(&active),
        ));

        let priority_clone = Arc::downgrade(&priority_scheduler);
        active.set_activate_successors_callback(Box::new(move |tx, block| {
            if let Some(priority) = priority_clone.upgrade() {
                priority.activate_successors(&tx, block);
            }
        }));

        let request_aggregator = Arc::new(RequestAggregator::new(
            config.clone(),
            Arc::clone(&stats),
            Arc::clone(&vote_generator),
            Arc::clone(&final_generator),
            Arc::clone(&history),
            Arc::clone(&ledger),
            Arc::clone(&wallets),
            Arc::clone(&active),
            network_params.network.is_dev_network(),
        ));
        request_aggregator.start();

        let backlog_population = Arc::new(BacklogPopulation::new(
            BacklogPopulationConfig {
                enabled: config.frontiers_confirmation != FrontiersConfirmationMode::Disabled,
                batch_size: config.backlog_scan_batch_size,
                frequency: config.backlog_scan_frequency,
            },
            Arc::clone(&ledger),
            Arc::clone(&stats),
        ));

        let ascendboot = Arc::new(BootstrapAscending::new(
            Arc::clone(&block_processor),
            Arc::clone(&ledger),
            Arc::clone(&stats),
            Arc::clone(&channels),
            config.clone(),
            network_params.network.clone(),
        ));

        let local_block_broadcaster = Arc::new(LocalBlockBroadcaster::new(
            Arc::clone(&block_processor),
            Arc::clone(&stats),
            Arc::clone(&channels),
            Arc::clone(&representative_register),
            Arc::clone(&ledger),
            Arc::clone(&confirming_set),
            !flags.disable_block_processor_republishing,
        ));
        local_block_broadcaster.initialize();

        let process_live_dispatcher = Arc::new(ProcessLiveDispatcher::new(
            Arc::clone(&ledger),
            Arc::clone(&priority_scheduler),
            websocket.clone(),
        ));

        let live_message_processor = Arc::new(LiveMessageProcessor::new(
            Arc::clone(&stats),
            Arc::clone(&channels),
            Arc::clone(&block_processor),
            config.clone(),
            flags.clone(),
            Arc::clone(&wallets),
            Arc::clone(&request_aggregator),
            Arc::clone(&vote_processor_queue),
            Arc::clone(&telemetry),
            Arc::clone(&bootstrap_server),
            Arc::clone(&ascendboot),
        ));

        let live_message_processor_weak = Arc::downgrade(&live_message_processor);
        *inbound_impl.write().unwrap() =
            Box::new(move |msg: DeserializedMessage, channel: Arc<ChannelEnum>| {
                if let Some(processor) = live_message_processor_weak.upgrade() {
                    processor.process(msg.message, &channel);
                }
            });

        let network_threads = Arc::new(NetworkThreads::new(
            Arc::clone(&channels),
            config.clone(),
            flags.clone(),
            network_params.clone(),
            Arc::clone(&stats),
            Arc::clone(&syn_cookies),
        ));

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
            vote_generator,
            final_generator,
            active,
            vote_processor,
            websocket,
            bootstrap_initiator,
            rep_crawler,
            tcp_listener,
            hinted_scheduler,
            manual_scheduler,
            optimistic_scheduler,
            priority_scheduler,
            request_aggregator,
            backlog_population,
            ascendboot,
            local_block_broadcaster,
            process_live_dispatcher,
            live_message_processor,
            network_threads,
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
