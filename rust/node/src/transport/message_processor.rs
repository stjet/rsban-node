use super::{InboundMessageQueue, LiveMessageProcessor, Network};
use crate::config::{NodeConfig, NodeFlags};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

/// Process inbound messages from other nodes
pub struct MessageProcessor {
    flags: NodeFlags,
    config: NodeConfig,
    processing_threads: Vec<JoinHandle<()>>,
    state: Arc<State>,
}

impl MessageProcessor {
    pub fn new(
        flags: NodeFlags,
        config: NodeConfig,
        inbound_queue: Arc<InboundMessageQueue>,
        live_message_processor: Arc<LiveMessageProcessor>,
    ) -> Self {
        Self {
            flags,
            config,
            processing_threads: Vec::new(),
            state: Arc::new(State {
                inbound_queue,
                live_message_processor,
                stopped: AtomicBool::new(false),
            }),
        }
    }

    pub fn start(&mut self) {
        if !self.flags.disable_tcp_realtime {
            for _ in 0..self.config.network_threads {
                let state = self.state.clone();
                self.processing_threads.push(
                    std::thread::Builder::new()
                        .name("Pkt processing".to_string())
                        .spawn(move || {
                            state.run();
                        })
                        .unwrap(),
                );
            }
        }
    }

    pub fn stop(&mut self) {
        self.state.stopped.store(true, Ordering::SeqCst);
        self.state.inbound_queue.stop();
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

struct State {
    stopped: AtomicBool,
    live_message_processor: Arc<LiveMessageProcessor>,
    inbound_queue: Arc<InboundMessageQueue>,
}

impl State {
    fn run(&self) {
        while !self.stopped.load(Ordering::SeqCst) {
            if let Some((message, channel)) = self.inbound_queue.next() {
                self.live_message_processor
                    .process(message.message, &channel);
            }
        }
    }
}
