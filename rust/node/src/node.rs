use crate::{
    block_processing::{
        BacklogPopulation, BacklogPopulationConfig, BlockProcessor, BlockSource,
        LocalBlockBroadcaster, LocalBlockBroadcasterExt, UncheckedMap,
    },
    bootstrap::{
        BootstrapAscending, BootstrapInitiator, BootstrapInitiatorExt,
        BootstrapMessageVisitorFactory, BootstrapServer,
    },
    cementation::ConfirmingSet,
    config::{FrontiersConfirmationMode, NodeConfig, NodeFlags},
    consensus::{
        AccountBalanceChangedCallback, ActiveTransactions, ActiveTransactionsExt, ElectionBehavior,
        ElectionEndCallback, ElectionStatusType, HintedScheduler, LocalVoteHistory,
        ManualScheduler, OptimisticScheduler, PriorityScheduler, ProcessLiveDispatcher,
        ProcessLiveDispatcherExt, RepTiers, RequestAggregator, RequestAggregatorExt, VoteCache,
        VoteGenerator, VoteProcessor, VoteProcessorQueue,
    },
    node_id_key_file::NodeIdKeyFile,
    representatives::{RepCrawler, RepresentativeRegister},
    stats::{DetailType, Direction, LedgerStats, StatType, Stats},
    transport::{
        BufferDropPolicy, ChannelEnum, InboundCallback, KeepaliveFactory, LiveMessageProcessor,
        NetworkFilter, NetworkThreads, OutboundBandwidthLimiter, SocketObserver, SynCookies,
        TcpChannels, TcpChannelsOptions, TcpListener, TcpListenerExt, TcpMessageManager,
        TrafficType,
    },
    utils::{
        AsyncRuntime, LongRunningTransactionLogger, ThreadPool, ThreadPoolImpl, TxnTrackingConfig,
    },
    wallets::{Wallets, WalletsExt},
    websocket::create_websocket_server,
    work::DistributedWorkFactory,
    NetworkParams, OnlineReps, OnlineWeightSampler, TelementryConfig, Telemetry, BUILD_INFO,
    VERSION_STRING,
};
use rsnano_core::{work::WorkPoolImpl, KeyPair, Vote, VoteCode};
use rsnano_ledger::Ledger;
use rsnano_messages::{ConfirmAck, DeserializedMessage, Message};
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
use tracing::{debug, error, info, warn};

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

        // empty `config.peering_port` means the user made no port choice at all;
        // otherwise, any value is considered, with `0` having the special meaning of 'let the OS pick a port instead'
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

        // BEWARE: `bootstrap` takes `network.port` instead of `config.peering_port` because when the user doesn't specify
        //         a peering port and wants the OS to pick one, the picking happens when `network` gets initialized
        //         (if UDP is active, otherwise it happens when `bootstrap` gets initialized), so then for TCP traffic
        //         we want to tell `bootstrap` to use the already picked port instead of itself picking a different one.
        //         Thus, be very careful if you change the order: if `bootstrap` gets constructed before `network`,
        //         the latter would inherit the port from the former (if TCP is active, otherwise `network` picks first)
        //
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

        let keepalive_factory = Arc::new(KeepaliveFactory {
            channels: Arc::clone(&channels),
            config: config.clone(),
        });
        let network_threads = Arc::new(NetworkThreads::new(
            Arc::clone(&channels),
            config.clone(),
            flags.clone(),
            network_params.clone(),
            Arc::clone(&stats),
            Arc::clone(&syn_cookies),
            Arc::clone(&keepalive_factory),
        ));

        let processor = Arc::downgrade(&live_message_processor);
        channels.set_sink(Box::new(move |msg, channel| {
            if let Some(processor) = processor.upgrade() {
                processor.process(msg.message, &channel);
            }
        }));

        debug!("Constructing node...");

        let manual_weak = Arc::downgrade(&manual_scheduler);
        wallets.set_start_election_callback(Box::new(move |block| {
            if let Some(manual) = manual_weak.upgrade() {
                manual.push(block, None, ElectionBehavior::Normal);
            }
        }));

        let rep_crawler_w = Arc::downgrade(&rep_crawler);
        if !flags.disable_rep_crawler {
            channels.on_new_channel(Arc::new(move |channel| {
                if let Some(crawler) = rep_crawler_w.upgrade() {
                    crawler.query_channel(channel);
                }
            }));
        }

        let block_processor_w = Arc::downgrade(&block_processor);
        let history_w = Arc::downgrade(&history);
        let active_w = Arc::downgrade(&active);
        block_processor.set_blocks_rolled_back_callback(Box::new(
            move |rolled_back, initial_block| {
                // Deleting from votes cache, stop active transaction
                let Some(block_processor) = block_processor_w.upgrade() else {
                    return;
                };
                let Some(history) = history_w.upgrade() else {
                    return;
                };
                let Some(active) = active_w.upgrade() else {
                    return;
                };
                for i in rolled_back {
                    block_processor.notify_block_rolled_back(&i);

                    history.erase(&i.root());
                    // Stop all rolled back active transactions except initial
                    if i.hash() != initial_block.hash() {
                        active.erase(&i.qualified_root());
                    }
                }
            },
        ));

        channels.set_observer(Arc::downgrade(&Arc::clone(&tcp_listener).as_observer()));

        let bootstrap_workers: Arc<dyn ThreadPool> = Arc::new(ThreadPoolImpl::create(
            config.bootstrap_serving_threads as usize,
            "Bootstrap work".to_string(),
        ));

        let visitor_factory = Arc::new(BootstrapMessageVisitorFactory::new(
            Arc::clone(&async_rt),
            Arc::clone(&syn_cookies),
            Arc::clone(&stats),
            network_params.network.clone(),
            Arc::new(node_id.clone()),
            Arc::clone(&ledger),
            Arc::clone(&bootstrap_workers),
            Arc::clone(&block_processor),
            Arc::clone(&bootstrap_initiator),
            flags.clone(),
        ));

        channels.set_message_visitor_factory(visitor_factory);

        process_live_dispatcher.connect(&block_processor);

        let block_processor_w = Arc::downgrade(&block_processor);
        unchecked.set_satisfied_observer(Box::new(move |info| {
            if let Some(processor) = block_processor_w.upgrade() {
                processor.add(
                    info.block.as_ref().unwrap().clone(),
                    BlockSource::Unchecked,
                    None,
                );
            }
        }));

        let priority_w = Arc::downgrade(&priority_scheduler);
        let optimistic_w = Arc::downgrade(&optimistic_scheduler);
        backlog_population.set_activate_callback(Box::new(move |tx, account, info, height| {
            let Some(priority) = priority_w.upgrade() else {
                return;
            };
            let Some(optimistic) = optimistic_w.upgrade() else {
                return;
            };
            priority.activate(account, tx);
            optimistic.activate(*account, info, height);
        }));

        let ledger_w = Arc::downgrade(&ledger);
        let vote_cache_w = Arc::downgrade(&vote_cache);
        let wallets_w = Arc::downgrade(&wallets);
        let channels_w = Arc::downgrade(&channels);
        active.add_vote_processed_observer(Box::new(move |vote, source, results| {
            let Some(ledger) = ledger_w.upgrade() else {
                return;
            };
            let Some(vote_cache) = vote_cache_w.upgrade() else {
                return;
            };
            let Some(wallets) = wallets_w.upgrade() else {
                return;
            };
            let Some(channels) = channels_w.upgrade() else {
                return;
            };
            let rep_weight = ledger.weight(&vote.voting_account);
            vote_cache
                .lock()
                .unwrap()
                .observe(vote, rep_weight, source, results.clone());

            // Republish vote if it is new and the node does not host a principal representative (or close to)
            let processed = results.iter().any(|(_, code)| *code == VoteCode::Vote);
            if processed {
                if wallets.should_republish_vote(vote.voting_account) {
                    let ack = Message::ConfirmAck(ConfirmAck::new(vote.as_ref().clone()));
                    channels.flood_message(&ack, 0.5);
                }
            }
        }));

        let priority_w = Arc::downgrade(&priority_scheduler);
        let hinted_w = Arc::downgrade(&hinted_scheduler);
        let optimistic_w = Arc::downgrade(&optimistic_scheduler);
        // Notify election schedulers when AEC frees election slot
        *active.vacancy_update.lock().unwrap() = Box::new(move || {
            let Some(priority) = priority_w.upgrade() else {
                return;
            };
            let Some(hinted) = hinted_w.upgrade() else {
                return;
            };
            let Some(optimistic) = optimistic_w.upgrade() else {
                return;
            };

            priority.notify();
            hinted.notify();
            optimistic.notify();
        });

        let keepalive_factory_w = Arc::downgrade(&keepalive_factory);
        channels.on_new_channel(Arc::new(move |channel| {
            let Some(factory) = keepalive_factory_w.upgrade() else {
                return;
            };
            let keepalive = factory.create_keepalive_self();
            let msg = Message::Keepalive(keepalive);
            channel.send(&msg, None, BufferDropPolicy::Limiter, TrafficType::Generic);
        }));

        // Add block confirmation type stats regardless of http-callback and websocket subscriptions
        let stats_w = Arc::downgrade(&stats);
        active.add_election_end_callback(Box::new(
            move |status, _votes, _account, _amount, _is_state_send, _is_state_epoch| {
                let Some(stats) = stats_w.upgrade() else {
                    return;
                };
                match status.election_status_type {
                    ElectionStatusType::ActiveConfirmedQuorum => stats.inc_dir(
                        StatType::ConfirmationObserver,
                        DetailType::ActiveQuorum,
                        Direction::Out,
                    ),
                    ElectionStatusType::ActiveConfirmationHeight => stats.inc_dir(
                        StatType::ConfirmationObserver,
                        DetailType::ActiveConfHeight,
                        Direction::Out,
                    ),
                    ElectionStatusType::InactiveConfirmationHeight => stats.inc_dir(
                        StatType::ConfirmationObserver,
                        DetailType::InactiveConfHeight,
                        Direction::Out,
                    ),
                    ElectionStatusType::Ongoing => unreachable!(),
                    ElectionStatusType::Stopped => {}
                }
            },
        ));

        let rep_crawler_w = Arc::downgrade(&rep_crawler);
        let online_reps_w = Arc::downgrade(&online_reps);
        vote_processor.add_vote_processed_callback(Box::new(move |vote, channel, code| {
            debug_assert!(code != VoteCode::Invalid);
            let Some(rep_crawler) = rep_crawler_w.upgrade() else {
                return;
            };
            let Some(online_reps) = online_reps_w.upgrade() else {
                return;
            };
            let active_in_rep_crawler = rep_crawler.process(Arc::clone(vote), Arc::clone(channel));
            if active_in_rep_crawler {
                // Representative is defined as online if replying to live votes or rep_crawler queries
                online_reps.lock().unwrap().observe(vote.voting_account);
            }
        }));

        let network_label = network_params.network.get_current_network_as_string();
        info!("Node starting, version: {}", VERSION_STRING);
        info!("Build information: {}", BUILD_INFO);
        info!("Active network: {}", network_label);
        info!("Database backend: {}", store.vendor());
        info!("Data path: {:?}", application_path);
        info!(
            "Work pool threads: {} ({})",
            work.thread_count(),
            if work.has_opencl() { "OpenCL" } else { "CPU" }
        );
        info!("Work peers: {}", config.work_peers.len());
        info!("Node ID: {}", node_id.public_key().to_node_id());

        if !distributed_work.work_generation_enabled() {
            info!("Work generation is disabled");
        }

        info!(
            "Outbound bandwidth limit: {} bytes/s, burst ratio: {}",
            config.bandwidth_limit, config.bandwidth_limit_burst_ratio
        );

        if !ledger.block_or_pruned_exists(&network_params.ledger.genesis.hash()) {
            error!("Genesis block not found. This commonly indicates a configuration issue, check that the --network or --data_path command line arguments are correct, and also the ledger backend node config option. If using a read-only CLI command a ledger must already exist, start the node with --daemon first.");

            if network_params.network.is_beta_network() {
                error!("Beta network may have reset, try clearing database files");
            }

            panic!("Genesis block not found!");
        }

        if config.enable_voting {
            info!(
                "Voting is enabled, more system resources will be used, local representatives: {}",
                wallets.voting_reps_count()
            );
            if wallets.voting_reps_count() > 1 {
                warn!("Voting with more than one representative can limit performance");
            }
        }

        Self {
            node_id,
            workers,
            bootstrap_workers,
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
