use crate::{node_factory::NodeFactory, nullable_runtime::NullableRuntime};
use num::FromPrimitive;
use num_derive::FromPrimitive;
use rsnano_core::Networks;
use rsnano_node::{Node, NodeCallbacks};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
};

#[derive(FromPrimitive, PartialEq, Eq)]
pub enum NodeState {
    Starting,
    Started,
    Stopping,
    Stopped,
}

pub(crate) struct NodeRunner {
    node_factory: NodeFactory,
    runtime: Arc<NullableRuntime>,
    node: Arc<Mutex<Option<Arc<Node>>>>,
    state: Arc<AtomicU8>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl NodeRunner {
    pub(crate) fn new(runtime: Arc<NullableRuntime>, node_factory: NodeFactory) -> Self {
        Self {
            node_factory,
            runtime,
            node: Arc::new(Mutex::new(None)),
            state: Arc::new(AtomicU8::new(NodeState::Stopped as u8)),
            stop: None,
        }
    }

    pub fn start_node(
        &mut self,
        network: Networks,
        data_path: impl Into<PathBuf>,
        callbacks: NodeCallbacks,
    ) {
        self.state
            .store(NodeState::Starting as u8, Ordering::SeqCst);

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        self.stop = Some(tx);

        let node_factory = self.node_factory.clone();
        let node = self.node.clone();
        let state1 = self.state.clone();
        let state2 = self.state.clone();
        let data_path = data_path.into();
        self.runtime.spawn(async move {
            node_factory
                .run_node(
                    network,
                    data_path,
                    callbacks,
                    move |n| {
                        *node.lock().unwrap() = Some(n);
                        state1.store(NodeState::Started as u8, Ordering::SeqCst);
                    },
                    async move {
                        let _ = rx.await;
                    },
                )
                .await;
            state2.store(NodeState::Stopped as u8, Ordering::SeqCst);
        });
    }

    pub(crate) fn stop(&mut self) {
        if let Some(tx) = self.stop.take() {
            self.state
                .store(NodeState::Stopping as u8, Ordering::SeqCst);
            let _ = tx.send(());
        }
    }

    pub(crate) fn state(&self) -> NodeState {
        FromPrimitive::from_u8(self.state.load(Ordering::SeqCst)).unwrap()
    }

    pub(crate) fn node(&self) -> Option<Arc<Node>> {
        self.node.lock().unwrap().clone()
    }
}

impl Drop for NodeRunner {
    fn drop(&mut self) {
        if let Some(tx_stop) = self.stop.take() {
            let _ = tx_stop.send(());
        }
    }
}
