use super::{Network, NetworkExt};
use crate::config::{NodeConfig, NodeFlags};
use std::{sync::Arc, thread::JoinHandle};

/// Process inbound messages from other nodes
pub struct MessageProcessor {
    flags: NodeFlags,
    config: NodeConfig,
    network: Arc<Network>,
    pub processing_threads: Vec<JoinHandle<()>>,
}

impl MessageProcessor {
    pub fn new(flags: NodeFlags, config: NodeConfig, network: Arc<Network>) -> Self {
        Self {
            flags,
            config,
            network,
            processing_threads: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        if !self.flags.disable_tcp_realtime {
            for _ in 0..self.config.network_threads {
                let network = Arc::clone(&self.network);
                self.processing_threads.push(
                    std::thread::Builder::new()
                        .name("Pkt processing".to_string())
                        .spawn(move || {
                            network.process_messages();
                        })
                        .unwrap(),
                );
            }
        }
    }

    pub fn stop(&mut self) {
        for t in self.processing_threads.drain(..) {
            t.join().unwrap();
        }
    }
}

impl Drop for MessageProcessor {
    fn drop(&mut self) {
        // All threads must be stopped before this destructor
        debug_assert!(self.processing_threads.is_empty());
    }
}
