use rsnano_core::Networks;
use rsnano_node::NodeBuilder;

pub(crate) struct NodeBuilderFactory {
    runtime: tokio::runtime::Handle,
    is_nulled: bool,
}

impl NodeBuilderFactory {
    pub(crate) fn new(runtime: tokio::runtime::Handle) -> Self {
        Self {
            runtime,
            is_nulled: false,
        }
    }

    pub(crate) fn new_null() -> Self {
        Self {
            runtime: tokio::runtime::Handle::current(),
            is_nulled: true,
        }
    }

    pub(crate) fn builder_for(&self, network: Networks) -> NodeBuilder {
        let builder = if self.is_nulled {
            NodeBuilder::new_null(network)
        } else {
            NodeBuilder::new(network)
        };
        builder.runtime(self.runtime.clone())
    }
}

impl Default for NodeBuilderFactory {
    fn default() -> Self {
        Self::new(tokio::runtime::Handle::current())
    }
}
