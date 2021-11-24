use std::net::Ipv6Addr;

use crate::{
    numbers::{Amount, GXRB_RATIO, XRB_RATIO},
    secure::NetworkParams,
    utils::{get_cpu_count, TomlWriter},
};
use anyhow::Result;

pub struct NodeConfig {
    pub peering_port: u16,
    pub bootstrap_fraction_numerator: u32,
    pub receive_minimum: Amount,
    pub online_weight_minimum: Amount,
    pub election_hint_weight_percent: u32,
    pub password_fanout: u32,
    pub io_threads: u32,
    pub network_threads: u32,
    pub work_threads: u32,
    pub signature_checker_threads: u32,
    pub enable_voting: bool,
    pub bootstrap_connections: u32,
    pub bootstrap_connections_max: u32,
    pub bootstrap_initiator_threads: u32,
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
}

impl NodeConfig {
    pub fn new(peering_port: u16, network_params: &NetworkParams) -> Self {
        Self {
            peering_port,
            bootstrap_fraction_numerator: 1,
            receive_minimum: Amount::new(*XRB_RATIO),
            online_weight_minimum: Amount::new(60000 * *GXRB_RATIO),
            election_hint_weight_percent: 10,
            password_fanout: 1024,
            io_threads: std::cmp::max(get_cpu_count() as u32, 4),
            network_threads: std::cmp::max(get_cpu_count() as u32, 4),
            work_threads: std::cmp::max(get_cpu_count() as u32, 4),
            /* Use half available threads on the system for signature checking. The calling thread does checks as well, so these are extra worker threads */
            signature_checker_threads: get_cpu_count() as u32 / 2,
            enable_voting: false,
            bootstrap_connections: 4,
            bootstrap_connections_max: 64,
            bootstrap_initiator_threads: 1,
            bootstrap_frontier_request_count: 1024 * 1024,
            block_processor_batch_max_time_ms: if network_params.network.is_dev_network() {
                500
            } else {
                5000
            },
            allow_local_peers: !(network_params.network.is_live_network()
                || network_params.network.is_test_network()), // disable by default for live network
            vote_minimum: Amount::new(*GXRB_RATIO),
            vote_generator_delay_ms: 100,
            vote_generator_threshold: 3,
            unchecked_cutoff_time_s: 4 * 60 * 60, // 4 hours
            tcp_io_timeout_s: if network_params.network.is_dev_network() {
                5
            } else {
                15
            },
            pow_sleep_interval_ns: 0,
            external_address: Ipv6Addr::UNSPECIFIED.to_string(),
            external_port: 0,
        }
    }

    pub fn serialize_toml(&self, toml: &mut impl TomlWriter) -> Result<()> {
        toml.put_u16(
            "peering_port",
            self.peering_port,
            "Node peering port.\ntype:uint16",
        )?;
        toml.put_u32("bootstrap_fraction_numerator", self.bootstrap_fraction_numerator, "Change bootstrap threshold (online stake / 256 * bootstrap_fraction_numerator).\ntype:uint32")?;
        toml.put_str("receive_minimum", &self.receive_minimum.to_string_dec (), "Minimum receive amount. Only affects node wallets. A large amount is recommended to avoid automatic work generation for tiny transactions.\ntype:string,amount,raw")?;
        toml.put_str("online_weight_minimum", &self.online_weight_minimum.to_string_dec (), "When calculating online weight, the node is forced to assume at least this much voting weight is online, thus setting a floor for voting weight to confirm transactions at online_weight_minimum * \"quorum delta\".\ntype:string,amount,raw")?;
        toml.put_u32("election_hint_weight_percent", self.election_hint_weight_percent, "Percentage of online weight to hint at starting an election. Defaults to 10.\ntype:uint32,[5,50]")?;
        toml.put_u32(
            "password_fanout",
            self.password_fanout,
            "Password fanout factor.\ntype:uint64",
        )?;
        toml.put_u32("io_threads", self.io_threads, "Number of threads dedicated to I/O operations. Defaults to the number of CPU threads, and at least 4.\ntype:uint64")?;
        toml.put_u32("network_threads", self.network_threads, "Number of threads dedicated to processing network messages. Defaults to the number of CPU threads, and at least 4.\ntype:uint64")?;
        toml.put_u32("work_threads", self.work_threads, "Number of threads dedicated to CPU generated work. Defaults to all available CPU threads.\ntype:uint64")?;
        toml.put_u32("signature_checker_threads", self.signature_checker_threads, "Number of additional threads dedicated to signature verification. Defaults to number of CPU threads / 2.\ntype:uint64")?;
        toml.put_bool("enable_voting", self.enable_voting, "Enable or disable voting. Enabling this option requires additional system resources, namely increased CPU, bandwidth and disk usage.\ntype:bool")?;
        toml.put_u32("bootstrap_connections", self.bootstrap_connections, "Number of outbound bootstrap connections. Must be a power of 2. Defaults to 4.\nWarning: a larger amount of connections may use substantially more system memory.\ntype:uint64")?;
        toml.put_u32("bootstrap_connections_max", self.bootstrap_connections_max, "Maximum number of inbound bootstrap connections. Defaults to 64.\nWarning: a larger amount of connections may use additional system memory.\ntype:uint64")?;
        toml.put_u32("bootstrap_initiator_threads", self.bootstrap_initiator_threads, "Number of threads dedicated to concurrent bootstrap attempts. Defaults to 1.\nWarning: a larger amount of attempts may use additional system memory and disk IO.\ntype:uint64")?;
        toml.put_u32("bootstrap_frontier_request_count", self.bootstrap_frontier_request_count, "Number frontiers per bootstrap frontier request. Defaults to 1048576.\ntype:uint32,[1024..4294967295]")?;
        toml.put_i64("block_processor_batch_max_time", self.block_processor_batch_max_time_ms, "The maximum time the block processor can continuously process blocks for.\ntype:milliseconds")?;
        toml.put_bool(
            "allow_local_peers",
            self.allow_local_peers,
            "Enable or disable local host peering.\ntype:bool",
        )?;
        toml.put_str("vote_minimum", &self.vote_minimum.to_string_dec (), "Local representatives do not vote if the delegated weight is under this threshold. Saves on system resources.\ntype:string,amount,raw")?;
        toml.put_i64("vote_generator_delay", self.vote_generator_delay_ms, "Delay before votes are sent to allow for efficient bundling of hashes in votes.\ntype:milliseconds")?;
        toml.put_u32("vote_generator_threshold", self.vote_generator_threshold, "Number of bundled hashes required for an additional generator delay.\ntype:uint64,[1..11]")?;
        toml.put_i64("unchecked_cutoff_time", self.unchecked_cutoff_time_s, "Number of seconds before deleting an unchecked entry.\nWarning: lower values (e.g., 3600 seconds, or 1 hour) may result in unsuccessful bootstraps, especially a bootstrap from scratch.\ntype:seconds")?;
        toml.put_i64("tcp_io_timeout", self.tcp_io_timeout_s , "Timeout for TCP connect-, read- and write operations.\nWarning: a low value (e.g., below 5 seconds) may result in TCP connections failing.\ntype:seconds")?;
        toml.put_i64 ("pow_sleep_interval", self.pow_sleep_interval_ns, "Time to sleep between batch work generation attempts. Reduces max CPU usage at the expense of a longer generation time.\ntype:nanoseconds")?;
        toml.put_str("external_address", &self.external_address, "The external address of this node (NAT). If not set, the node will request this information via UPnP.\ntype:string,ip")?;
        toml.put_u16("external_port", self.external_port, "The external port number of this node (NAT). Only used if external_address is set.\ntype:uint16")?;
        // toml.put ("tcp_incoming_connections_max", tcp_incoming_connections_max, "Maximum number of incoming TCP connections.\ntype:uint64");
        // toml.put ("use_memory_pools", use_memory_pools, "If true, allocate memory from memory pools. Enabling this may improve performance. Memory is never released to the OS.\ntype:bool");
        // toml.put ("confirmation_history_size", confirmation_history_size, "Maximum confirmation history size. If tracking the rate of block confirmations, the websocket feature is recommended instead.\ntype:uint64");
        // toml.put ("active_elections_size", active_elections_size, "Number of active elections. Elections beyond this limit have limited survival time.\nWarning: modifying this value may result in a lower confirmation rate.\ntype:uint64,[250..]");
        // toml.put ("bandwidth_limit", bandwidth_limit, "Outbound traffic limit in bytes/sec after which messages will be dropped.\nNote: changing to unlimited bandwidth (0) is not recommended for limited connections.\ntype:uint64");
        // toml.put ("bandwidth_limit_burst_ratio", bandwidth_limit_burst_ratio, "Burst ratio for outbound traffic shaping.\ntype:double");
        // toml.put ("conf_height_processor_batch_min_time", conf_height_processor_batch_min_time.count (), "Minimum write batching time when there are blocks pending confirmation height.\ntype:milliseconds");
        // toml.put ("backup_before_upgrade", backup_before_upgrade, "Backup the ledger database before performing upgrades.\nWarning: uses more disk storage and increases startup time when upgrading.\ntype:bool");
        // toml.put ("max_work_generate_multiplier", max_work_generate_multiplier, "Maximum allowed difficulty multiplier for work generation.\ntype:double,[1..]");

        Ok(())
    }
}
