use crate::{
    config::NodeConfig,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, TcpChannels, TcpChannelsExtension, TrafficType},
    utils::{into_ipv6_socket_address, AsyncRuntime},
    NetworkParams, OnlineReps,
};
use rsnano_core::BlockHash;
use rsnano_messages::{Keepalive, Message};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{debug, error};

use super::RepresentativeRegister;

pub struct RepCrawler {
    rep_crawler_impl: Mutex<RepCrawlerImpl>,
    representative_register: Arc<Mutex<RepresentativeRegister>>,
    online_reps: Arc<Mutex<OnlineReps>>,
    stats: Arc<Stats>,
    config: NodeConfig,
    network_params: NetworkParams,
    tcp_channels: Arc<TcpChannels>,
    async_rt: Arc<AsyncRuntime>,
}

impl RepCrawler {
    pub fn new(
        representative_register: Arc<Mutex<RepresentativeRegister>>,
        stats: Arc<Stats>,
        query_timeout: Duration,
        online_reps: Arc<Mutex<OnlineReps>>,
        config: NodeConfig,
        network_params: NetworkParams,
        tcp_channels: Arc<TcpChannels>,
        async_rt: Arc<AsyncRuntime>,
    ) -> Self {
        Self {
            representative_register: Arc::clone(&representative_register),
            online_reps,
            stats: Arc::clone(&stats),
            config,
            network_params,
            tcp_channels,
            async_rt,
            rep_crawler_impl: Mutex::new(RepCrawlerImpl {
                queries: OrderedQueries::new(),
                representative_register,
                stats,
                query_timeout,
                stopped: false,
            }),
        }
    }

    pub fn run(&self) {
        let guard = self.rep_crawler_impl.lock().unwrap();
        while !guard.stopped {
            drop(guard);

            let current_total_weight = self.representative_register.lock().unwrap().total_weight();
            let sufficient_weight = current_total_weight > self.online_reps.lock().unwrap().delta();

            // If online weight drops below minimum, reach out to preconfigured peers
            if !sufficient_weight {
                self.stats
                    .inc(StatType::RepCrawler, DetailType::Keepalive, Direction::In);
                self.keepalive_preconfigured();
            }
            todo!()
        }
    }

    pub fn keepalive_preconfigured(&self) {
        for peer in &self.config.preconfigured_peers {
            // can't use `network.port` here because preconfigured peers are referenced
            // just by their address, so we rely on them listening on the default port
            self.keepalive_or_connect(peer.clone(), self.network_params.network.default_node_port);
        }
    }

    pub fn keepalive_or_connect(&self, address: String, port: u16) {
        let channels = Arc::clone(&self.tcp_channels);
        self.async_rt.tokio.spawn(async move {
            match tokio::net::lookup_host((address.as_str(), port)).await {
                Ok(addresses) => {
                    for address in addresses {
                        let endpoint = into_ipv6_socket_address(address);
                        match channels.find_channel(&endpoint) {
                            Some(channel) => {
                                let keepalive = channels.create_keepalive_message();
                                channel.send(
                                    &keepalive,
                                    None,
                                    BufferDropPolicy::Limiter,
                                    TrafficType::Generic,
                                )
                            }
                            None => {
                                channels.start_tcp(endpoint);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Error resolving address for keepalive: {}:{} ({})",
                        address, port, e
                    )
                }
            }
        });
    }
}

struct RepCrawlerImpl {
    queries: OrderedQueries,
    representative_register: Arc<Mutex<RepresentativeRegister>>,
    stats: Arc<Stats>,
    query_timeout: Duration,
    stopped: bool,
}

impl RepCrawlerImpl {
    fn cleanup(&mut self) {
        // Evict reps with dead channels
        self.representative_register.lock().unwrap().cleanup_reps();

        // Evict queries that haven't been responded to in a while
        self.queries.retain(|query| {
            if query.time.elapsed() < self.query_timeout {
                return true; // Retain
            }

            if query.replies == 0 {
                debug!(
                    "Aborting unresponsive query for block {} from {}",
                    query.hash,
                    query.channel.remote_endpoint()
                );
                self.stats.inc(
                    StatType::RepCrawler,
                    DetailType::QueryTimeout,
                    Direction::In,
                );
            } else {
                debug!(
                    "Completion of query with {} replies for block {} from {}",
                    query.replies,
                    query.hash,
                    query.channel.remote_endpoint()
                );
                self.stats.inc(
                    StatType::RepCrawler,
                    DetailType::QueryCompletion,
                    Direction::In,
                );
            }

            false // Retain
        });
    }
}

struct QueryEntry {
    hash: BlockHash,
    channel: Arc<ChannelEnum>,
    time: Instant,
    /// number of replies to the query
    replies: usize,
}

struct OrderedQueries {
    entries: HashMap<usize, QueryEntry>,
    sequenced: Vec<usize>,
    by_channel: HashMap<usize, Vec<usize>>,
    by_hash: HashMap<BlockHash, Vec<usize>>,
    next_id: usize,
}

impl OrderedQueries {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            sequenced: Vec::new(),
            by_channel: HashMap::new(),
            by_hash: HashMap::new(),
            next_id: 1,
        }
    }

    fn insert(&mut self, entry: QueryEntry) {
        let entry_id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.sequenced.push(entry_id);
        self.by_channel
            .entry(entry.channel.channel_id())
            .or_default()
            .push(entry_id);
        self.by_hash.entry(entry.hash).or_default().push(entry_id);
        self.entries.insert(entry_id, entry);
    }

    fn retain(&mut self, predicate: impl Fn(&QueryEntry) -> bool) {
        let mut to_delete = Vec::new();
        for (&id, entry) in &self.entries {
            if predicate(entry) {
                to_delete.push(id);
            }
        }
        for id in to_delete {
            self.remove(id);
        }
    }

    fn remove(&mut self, entry_id: usize) {
        if let Some(entry) = self.entries.remove(&entry_id) {
            self.sequenced.retain(|id| *id != entry_id);
            self.by_channel.remove(&entry.channel.channel_id());
            self.by_hash.remove(&entry.hash);
        }
    }
}
