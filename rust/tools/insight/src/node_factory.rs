use rsnano_core::Networks;
use rsnano_daemon::DaemonBuilder;
use rsnano_node::{Node, NodeCallbacks};
use std::{future::Future, path::PathBuf, sync::Arc};

#[derive(Clone)]
pub(crate) struct NodeFactory {
    is_nulled: bool,
}

impl NodeFactory {
    pub(crate) fn new() -> Self {
        Self { is_nulled: false }
    }

    #[allow(dead_code)]
    pub(crate) fn new_null() -> Self {
        Self { is_nulled: true }
    }

    pub(crate) async fn run_node(
        &self,
        network: Networks,
        data_path: impl Into<PathBuf>,
        callbacks: NodeCallbacks,
        mut started: impl FnMut(Arc<Node>) + Send + 'static,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) {
        if self.is_nulled {
            let node = Arc::new(Node::new_null_with_callbacks(callbacks));
            started(node);
            shutdown.await;
        } else {
            DaemonBuilder::new(network)
                .data_path(data_path)
                .callbacks(callbacks)
                .on_node_started(started)
                .run(shutdown)
                .await;
        }
    }
}

impl Default for NodeFactory {
    fn default() -> Self {
        Self::new()
    }
}
