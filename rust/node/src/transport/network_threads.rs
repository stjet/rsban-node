use rsnano_messages::{Keepalive, Message};

use super::{SynCookies, TcpChannels, TcpChannelsExtension};
use crate::{
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, StatType, Stats},
    NetworkParams,
};
use std::{
    net::{Ipv6Addr, SocketAddrV6},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, SystemTime},
};

pub struct NetworkThreads {
    cleanup_thread: Option<JoinHandle<()>>,
    keepalive_thread: Option<JoinHandle<()>>,
    reachout_thread: Option<JoinHandle<()>>,
    processing_threads: Vec<JoinHandle<()>>,
    stopped: Arc<(Condvar, Mutex<bool>)>,
    channels: Arc<TcpChannels>,
    config: NodeConfig,
    flags: NodeFlags,
    network_params: NetworkParams,
    stats: Arc<Stats>,
    syn_cookies: Arc<SynCookies>,
}

impl NetworkThreads {
    pub fn new(
        channels: Arc<TcpChannels>,
        config: NodeConfig,
        flags: NodeFlags,
        network_params: NetworkParams,
        stats: Arc<Stats>,
        syn_cookies: Arc<SynCookies>,
    ) -> Self {
        Self {
            cleanup_thread: None,
            keepalive_thread: None,
            reachout_thread: None,
            processing_threads: Vec::new(),
            stopped: Arc::new((Condvar::new(), Mutex::new(false))),
            channels,
            config,
            flags,
            network_params,
            stats,
            syn_cookies,
        }
    }

    pub fn start(&mut self) {
        let cleanup = CleanupLoop {
            stopped: self.stopped.clone(),
            network_params: self.network_params.clone(),
            stats: Arc::clone(&self.stats),
            flags: self.flags.clone(),
            syn_cookies: Arc::clone(&self.syn_cookies),
            channels: Arc::clone(&self.channels),
        };

        self.cleanup_thread = Some(
            std::thread::Builder::new()
                .name("Net cleanup".to_string())
                .spawn(move || cleanup.run())
                .unwrap(),
        );

        let keepalive = KeepaliveLoop {
            stopped: self.stopped.clone(),
            channels: Arc::clone(&self.channels),
            network_params: self.network_params.clone(),
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
        };

        self.keepalive_thread = Some(
            std::thread::Builder::new()
                .name("Net keepalive".to_string())
                .spawn(move || keepalive.run())
                .unwrap(),
        );

        let reachout = ReachoutLoop {
            stopped: self.stopped.clone(),
            network_params: self.network_params.clone(),
            stats: Arc::clone(&self.stats),
            channels: Arc::clone(&self.channels),
        };

        self.reachout_thread = Some(
            std::thread::Builder::new()
                .name("Net reachout".to_string())
                .spawn(move || reachout.run())
                .unwrap(),
        );

        if !self.flags.disable_tcp_realtime {
            for _ in 0..self.config.network_threads {
                let channels = Arc::clone(&self.channels);
                self.processing_threads.push(
                    std::thread::Builder::new()
                        .name("Pkt processing".to_string())
                        .spawn(move || {
                            channels.process_messages();
                        })
                        .unwrap(),
                );
            }
        }
    }
    pub fn stop(&mut self) {
        *self.stopped.1.lock().unwrap() = true;
        self.stopped.0.notify_all();
        self.channels.stop();
        for t in self.processing_threads.drain(..) {
            t.join().unwrap();
        }
        if let Some(t) = self.keepalive_thread.take() {
            t.join().unwrap();
        }
        if let Some(t) = self.cleanup_thread.take() {
            t.join().unwrap();
        }
        if let Some(t) = self.reachout_thread.take() {
            t.join().unwrap();
        }
    }
}

impl Drop for NetworkThreads {
    fn drop(&mut self) {
        // All threads must be stopped before this destructor
        debug_assert!(self.processing_threads.is_empty());
        debug_assert!(self.cleanup_thread.is_none());
        debug_assert!(self.keepalive_thread.is_none());
    }
}

struct CleanupLoop {
    stopped: Arc<(Condvar, Mutex<bool>)>,
    network_params: NetworkParams,
    stats: Arc<Stats>,
    flags: NodeFlags,
    syn_cookies: Arc<SynCookies>,
    channels: Arc<TcpChannels>,
}

impl CleanupLoop {
    fn run(&self) {
        let mut stopped = self.stopped.1.lock().unwrap();
        while !*stopped {
            let timeout = if self.network_params.network.is_dev_network() {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(5)
            };
            stopped = self.stopped.0.wait_timeout(stopped, timeout).unwrap().0;

            if *stopped {
                return;
            }
            drop(stopped);

            self.stats.inc(StatType::Network, DetailType::LoopCleanup);

            if !self.flags.disable_connection_cleanup {
                self.channels.purge(
                    SystemTime::now()
                        - Duration::from_secs(self.network_params.network.cleanup_cutoff_s() as u64),
                );
            }

            self.syn_cookies.purge(Duration::from_secs(
                self.network_params.network.sync_cookie_cutoff_s as u64,
            ));

            stopped = self.stopped.1.lock().unwrap();
        }
    }
}

struct KeepaliveLoop {
    stopped: Arc<(Condvar, Mutex<bool>)>,
    network_params: NetworkParams,
    stats: Arc<Stats>,
    channels: Arc<TcpChannels>,
    config: NodeConfig,
}

impl KeepaliveLoop {
    fn run(&self) {
        let mut stopped = self.stopped.1.lock().unwrap();
        while !*stopped {
            stopped = self
                .stopped
                .0
                .wait_timeout(stopped, self.network_params.network.keepalive_period)
                .unwrap()
                .0;

            if *stopped {
                return;
            }
            drop(stopped);

            self.stats.inc(StatType::Network, DetailType::LoopKeepalive);
            self.flood_keepalive(0.75);
            self.flood_keepalive_self(0.25);

            self.channels.keepalive();

            stopped = self.stopped.1.lock().unwrap();
        }
    }

    fn flood_keepalive(&self, scale: f32) {
        let mut keepalive = Keepalive::default();
        self.channels.random_fill(&mut keepalive.peers);
        self.channels
            .flood_message(&Message::Keepalive(keepalive), scale);
    }

    fn flood_keepalive_self(&self, scale: f32) {
        let keepalive = self.keepalive_self();
        self.channels
            .flood_message(&Message::Keepalive(keepalive), scale);
    }

    fn keepalive_self(&self) -> Keepalive {
        let mut result = Keepalive::default();
        self.channels.random_fill(&mut result.peers);
        // We will clobber values in index 0 and 1 and if there are only 2 nodes in the system, these are the only positions occupied
        // Move these items to index 2 and 3 so they propagate
        result.peers[2] = result.peers[0];
        result.peers[3] = result.peers[1];
        // Replace part of message with node external address or listening port
        result.peers[1] = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0); // For node v19 (response channels)
        if self.config.external_address != Ipv6Addr::UNSPECIFIED.to_string()
            && self.config.external_port != 0
        {
            result.peers[0] = SocketAddrV6::new(
                self.config.external_address.parse().unwrap(),
                self.config.external_port,
                0,
                0,
            );
        } else {
            // TODO Read external address from port_mapping!
            //let external_address  node.port_mapping.external_address ());
            let external_address = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0);
            if !external_address.ip().is_unspecified() {
                result.peers[0] =
                    SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, self.channels.port(), 0, 0);
                result.peers[1] = external_address;
            } else {
                result.peers[0] =
                    SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, self.channels.port(), 0, 0);
            }
        }
        result
    }
}

struct ReachoutLoop {
    stopped: Arc<(Condvar, Mutex<bool>)>,
    network_params: NetworkParams,
    stats: Arc<Stats>,
    channels: Arc<TcpChannels>,
}

impl ReachoutLoop {
    fn run(&self) {
        let mut stopped = self.stopped.1.lock().unwrap();
        while !*stopped {
            stopped = self
                .stopped
                .0
                .wait_timeout(stopped, self.network_params.network.merge_period)
                .unwrap()
                .0;

            if *stopped {
                return;
            }
            drop(stopped);

            self.stats.inc(StatType::Network, DetailType::LoopReachout);

            if let Some(keepalive) = self.channels.sample_keepalive() {
                for peer in keepalive.peers {
                    self.channels.merge_peer(&peer);

                    // Throttle reachout attempts
                    std::thread::sleep(self.network_params.network.merge_period);
                }
            }

            stopped = self.stopped.1.lock().unwrap();
        }
    }
}
