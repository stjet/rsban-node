use super::{websocket_config::WebsocketConfig, DiagnosticsConfig, Networks};
use crate::{
    block_processing::{
        BacklogPopulationConfig, BlockProcessorConfig, LocalBlockBroadcasterConfig,
    },
    bootstrap::{BootstrapAscendingConfig, BootstrapInitiatorConfig, BootstrapServerConfig},
    cementation::ConfirmingSetConfig,
    consensus::{
        ActiveElectionsConfig, HintedSchedulerConfig, OptimisticSchedulerConfig,
        PriorityBucketConfig, RequestAggregatorConfig, VoteCacheConfig, VoteProcessorConfig,
    },
    stats::StatsConfig,
    transport::{MessageProcessorConfig, TcpConfig},
    IpcConfig, NetworkParams, DEV_NETWORK_PARAMS,
};
use once_cell::sync::Lazy;
use rand::{thread_rng, Rng};
use rsban_core::{
    utils::{get_env_or_default_string, is_sanitizer_build, Peer},
    Account, Amount, PublicKey,
};
use rsban_store_lmdb::LmdbConfig;
use std::{cmp::max, net::Ipv6Addr, time::Duration};

#[derive(Clone, Debug, PartialEq)]
pub struct NodeConfig {
    pub peering_port: Option<u16>,
    pub default_peering_port: u16,
    pub optimistic_scheduler: OptimisticSchedulerConfig,
    pub hinted_scheduler: HintedSchedulerConfig,
    pub priority_bucket: PriorityBucketConfig,
    pub bootstrap_fraction_numerator: u32,
    pub receive_minimum: Amount,
    pub online_weight_minimum: Amount,
    /// The minimum vote weight that a representative must have for its vote to be counted.
    /// All representatives above this weight will be kept in memory!
    pub representative_vote_weight_minimum: Amount,
    pub password_fanout: u32,
    pub io_threads: u32,
    pub network_threads: u32,
    pub work_threads: u32,
    pub background_threads: u32,
    pub signature_checker_threads: u32,
    pub enable_voting: bool,
    pub enable_vote_processor: bool,
    pub bootstrap_connections: u32,
    pub bootstrap_connections_max: u32,
    pub bootstrap_initiator_threads: u32,
    pub bootstrap_serving_threads: u32,
    pub bootstrap_frontier_request_count: u32,
    pub block_processor_batch_max_time_ms: i64,
    pub allow_local_peers: bool,
    pub vote_minimum: Amount,
    pub vote_generator_delay_ms: i64,
    pub vote_generator_threshold: u32,
    pub unchecked_cutoff_time_s: i64,
    pub tcp_io_timeout_s: i64,
    pub pow_sleep_interval_ns: i64,
    pub external_address: String,
    pub external_port: u16,
    pub tcp_incoming_connections_max: u32,
    pub use_memory_pools: bool,
    pub bandwidth_limit: usize,
    pub bandwidth_limit_burst_ratio: f64,
    pub max_peers_per_ip: u16,
    pub max_peers_per_subnetwork: u16,
    pub bootstrap_ascending: BootstrapAscendingConfig,
    pub bootstrap_server: BootstrapServerConfig,
    pub bootstrap_bandwidth_limit: usize,
    pub bootstrap_bandwidth_burst_ratio: f64,
    pub confirming_set_batch_time: Duration,
    pub backup_before_upgrade: bool,
    pub max_work_generate_multiplier: f64,
    pub max_queued_requests: u32,
    pub request_aggregator_threads: u32,
    pub max_unchecked_blocks: u32,
    pub rep_crawler_weight_minimum: Amount,
    pub work_peers: Vec<Peer>,
    pub secondary_work_peers: Vec<Peer>,
    pub preconfigured_peers: Vec<Peer>,
    pub preconfigured_representatives: Vec<PublicKey>,
    pub max_pruning_age_s: i64,
    pub max_pruning_depth: u64,
    pub callback_address: String,
    pub callback_port: u16,
    pub callback_target: String,
    pub websocket_config: WebsocketConfig,
    pub ipc_config: IpcConfig,
    pub diagnostics_config: DiagnosticsConfig,
    pub stat_config: StatsConfig,
    pub lmdb_config: LmdbConfig,
    pub vote_cache: VoteCacheConfig,
    pub rep_crawler_query_timeout: Duration,
    pub block_processor: BlockProcessorConfig,
    pub active_elections: ActiveElectionsConfig,
    pub vote_processor: VoteProcessorConfig,
    pub tcp: TcpConfig,
    pub request_aggregator: RequestAggregatorConfig,
    pub message_processor: MessageProcessorConfig,
    pub priority_scheduler_enabled: bool,
    pub local_block_broadcaster: LocalBlockBroadcasterConfig,
    pub confirming_set: ConfirmingSetConfig,
    pub monitor: MonitorConfig,
    pub backlog: BacklogPopulationConfig,
    pub network_duplicate_filter_cutoff: u64,
}

static DEFAULT_LIVE_PEER_NETWORK: Lazy<String> =
    Lazy::new(|| get_env_or_default_string("NANO_DEFAULT_PEER", "livenet.banano.cc"));

static DEFAULT_BETA_PEER_NETWORK: Lazy<String> =
    Lazy::new(|| get_env_or_default_string("NANO_DEFAULT_PEER", "livenet-beta.banano.cc"));

static DEFAULT_TEST_PEER_NETWORK: Lazy<String> =
    Lazy::new(|| get_env_or_default_string("NANO_DEFAULT_PEER", "peering-test.nano.org"));

impl NodeConfig {
    pub fn default_for(network: Networks, parallelism: usize) -> Self {
        let net_params = NetworkParams::new(network);
        Self::new(
            Some(net_params.network.default_node_port),
            &net_params,
            parallelism,
        )
    }

    pub fn new(
        peering_port: Option<u16>,
        network_params: &NetworkParams,
        parallelism: usize,
    ) -> Self {
        if peering_port == Some(0) {
            // comment for posterity:
            // - we used to consider ports being 0 a sentinel that meant to use a default port for that specific purpose
            // - the actual default value was determined based on the active network (e.g. dev network peering port = 44000)
            // - now, the 0 value means something different instead: user wants to let the OS pick a random port
            // - for the specific case of the peering port, after it gets picked, it can be retrieved by client code via
            //   node.network.endpoint ().port ()
            // - the config value does not get back-propagated because it represents the choice of the user, and that was 0
        }

        let mut enable_voting = false;
        let mut preconfigured_peers = Vec::new();
        let mut preconfigured_representatives = Vec::new();
        let default_port = network_params.network.default_node_port;
        match network_params.network.current_network {
            Networks::BananoDevNetwork => {
                enable_voting = true;
                preconfigured_representatives.push(network_params.ledger.genesis_account.into());
            }
            Networks::BananoBetaNetwork => {
                preconfigured_peers
                    .push(Peer::new(DEFAULT_BETA_PEER_NETWORK.clone(), default_port));
                preconfigured_representatives.push(
                    Account::decode_account(
                        "ban_1defau1t9off1ine9rep99999999999999999999999999999999wgmuzxxy",
                    )
                    .unwrap()
                    .into(),
                );
            }
            // https://github.com/BananoCoin/banano/blob/c1f8405d1994f55bc47d63dfe0fe20728e8856cb/nano/node/nodeconfig.cpp#L61
            Networks::BananoLiveNetwork => {
                preconfigured_peers
                    .push(Peer::new(DEFAULT_LIVE_PEER_NETWORK.clone(), default_port));
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "36B3AFC042CCB5099DC163FA2BFE42D6E486991B685EAAB0DF73714D91A59400",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "1ED5D232FA96BFA78D802F1F0EC2F487072B782A2E627B1CBCAFFFA640A0D2A9",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "E90AAA91A67EBFCF29B3764F212E802D2A5EE32D7E6DB2E2547484E69B88548F",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "3D94D2217FA5E8CEFCE1E95E7E6A53CD01E6E0C96ACF4D22A64A0D9A39BDB978",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "55155530A47B3A52F484309CABC26071BAF38522A599B6C0798F8037D8439F0C",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "490086E62B376C0EFBAA6AF9C41269EE7D723F98B4667416F075951E981E3F37",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "3EB5D6D3DF10CD7BF0AD46688232011B5F38359CB036B82CE9B58BCCC2ADA5F5",
                    )
                    .unwrap(),
                );
                preconfigured_representatives.push(
                    PublicKey::decode_hex(
                        "CD1182912CDE444D41A88BB5E7B144659E6F140CC0EE92D07B64CED59419D811",
                    )
                    .unwrap(),
                );
            }
            Networks::BananoTestNetwork => {
                preconfigured_peers
                    .push(Peer::new(DEFAULT_TEST_PEER_NETWORK.clone(), default_port));
                preconfigured_representatives.push(network_params.ledger.genesis_account.into());
            }
            Networks::Invalid => panic!("invalid network"),
        }

        let bootstrap_initiator_cfg =
            BootstrapInitiatorConfig::default_for(network_params.network.current_network);

        let block_processor_cfg = BlockProcessorConfig::new(network_params.work.clone());

        Self {
            peering_port,
            default_peering_port: network_params.network.default_node_port,
            bootstrap_fraction_numerator: 1,
            receive_minimum: Amount::micronano(1),
            online_weight_minimum: Amount::nano(60_000_000),
            representative_vote_weight_minimum: Amount::nano(10),
            password_fanout: 1024,
            io_threads: max(parallelism, 4) as u32,
            network_threads: max(parallelism, 4) as u32,
            work_threads: max(parallelism, 4) as u32,
            background_threads: max(parallelism, 4) as u32,
            /* Use half available threads on the system for signature checking. The calling thread does checks as well, so these are extra worker threads */
            signature_checker_threads: (parallelism / 2) as u32,
            enable_voting,
            enable_vote_processor: true,
            bootstrap_connections: bootstrap_initiator_cfg.bootstrap_connections,
            bootstrap_connections_max: bootstrap_initiator_cfg.bootstrap_connections_max,
            bootstrap_initiator_threads: 1,
            bootstrap_serving_threads: 1,
            bootstrap_frontier_request_count: bootstrap_initiator_cfg.frontier_request_count,
            block_processor_batch_max_time_ms: block_processor_cfg.batch_max_time.as_millis()
                as i64,
            allow_local_peers: !(network_params.network.is_live_network()
                || network_params.network.is_test_network()), // disable by default for live network
            vote_minimum: Amount::nano(1000),
            vote_generator_delay_ms: 100,
            vote_generator_threshold: 3,
            unchecked_cutoff_time_s: 4 * 60 * 60, // 4 hours
            tcp_io_timeout_s: if network_params.network.is_dev_network() && !is_sanitizer_build() {
                5
            } else {
                15
            },
            pow_sleep_interval_ns: 0,
            external_address: Ipv6Addr::UNSPECIFIED.to_string(),
            external_port: 0,
            // Default maximum incoming TCP connections, including realtime network & bootstrap
            tcp_incoming_connections_max: 2048,
            use_memory_pools: true,
            // Default outbound traffic shaping is 10MB/s
            bandwidth_limit: 10 * 1024 * 1024,
            // By default, allow bursts of 15MB/s (not sustainable)
            bandwidth_limit_burst_ratio: 3_f64,
            max_peers_per_ip: network_params.network.max_peers_per_ip as u16,
            max_peers_per_subnetwork: network_params.network.max_peers_per_subnetwork as u16,
            // Default boostrap outbound traffic limit is 5MB/s
            bootstrap_bandwidth_limit: 5 * 1024 * 1024,
            // Bootstrap traffic does not need bursts
            bootstrap_bandwidth_burst_ratio: 1.,
            bootstrap_ascending: Default::default(),
            bootstrap_server: Default::default(),
            confirming_set_batch_time: Duration::from_millis(250),
            backup_before_upgrade: false,
            max_work_generate_multiplier: 64_f64,
            max_queued_requests: 512,
            request_aggregator_threads: max(parallelism, 4) as u32,
            max_unchecked_blocks: 65536,
            rep_crawler_weight_minimum: Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .unwrap(),
            work_peers: Vec::new(),
            secondary_work_peers: vec![Peer::new("127.0.0.1", 8076)],
            preconfigured_peers,
            preconfigured_representatives,
            max_pruning_age_s: if !network_params.network.is_beta_network() {
                24 * 60 * 60
            } else {
                5 * 60
            }, // 1 day; 5 minutes for beta network
            max_pruning_depth: 0,
            callback_address: String::new(),
            callback_port: 0,
            callback_target: String::new(),
            websocket_config: WebsocketConfig::new(&network_params.network),
            ipc_config: IpcConfig::new(&network_params.network),
            diagnostics_config: DiagnosticsConfig::new(),
            stat_config: StatsConfig::new(),
            lmdb_config: LmdbConfig::new(),
            optimistic_scheduler: OptimisticSchedulerConfig::new(),
            hinted_scheduler: if network_params.network.is_dev_network() {
                HintedSchedulerConfig::default_for_dev_network()
            } else {
                HintedSchedulerConfig::default()
            },
            priority_bucket: Default::default(),
            vote_cache: Default::default(),
            active_elections: Default::default(),
            rep_crawler_query_timeout: if network_params.network.is_dev_network() {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(60)
            },
            block_processor: block_processor_cfg,
            vote_processor: VoteProcessorConfig::new(parallelism),
            tcp: if network_params.network.is_dev_network() {
                TcpConfig::for_dev_network()
            } else {
                Default::default()
            },
            request_aggregator: RequestAggregatorConfig::new(parallelism),
            message_processor: MessageProcessorConfig::new(parallelism),
            priority_scheduler_enabled: true,
            local_block_broadcaster: LocalBlockBroadcasterConfig::new(
                network_params.network.current_network,
            ),
            confirming_set: Default::default(),
            monitor: Default::default(),
            backlog: Default::default(),
            network_duplicate_filter_cutoff: 60,
        }
    }

    pub fn new_test_instance() -> Self {
        Self::new(None, &DEV_NETWORK_PARAMS, 1)
    }

    pub fn random_representative(&self) -> PublicKey {
        let i = thread_rng().gen_range(0..self.preconfigured_representatives.len());
        return self.preconfigured_representatives[i];
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MonitorConfig {
    pub enabled: bool,
    pub interval: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(60),
        }
    }
}
