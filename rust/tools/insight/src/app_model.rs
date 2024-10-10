use crate::{node_builder_factory::NodeBuilderFactory, nullable_runtime::NullableRuntime};
use num_format::{Locale, ToFormattedString};
use rsnano_core::Networks;
use rsnano_node::{Node, NodeCallbacks, NodeExt};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

pub(crate) struct AppModel {
    runtime: Arc<NullableRuntime>,
    node_builder_factory: NodeBuilderFactory,
    node: Option<Arc<Node>>,
    started: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    published: Arc<AtomicUsize>,
}

impl AppModel {
    pub(crate) fn new(
        runtime: Arc<NullableRuntime>,
        node_builder_factory: NodeBuilderFactory,
    ) -> Self {
        Self {
            node_builder_factory,
            runtime,
            node: None,
            started: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(true)),
            published: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn with_runtime(runtime: tokio::runtime::Handle) -> Self {
        Self::new(
            Arc::new(NullableRuntime::new(runtime.clone())),
            NodeBuilderFactory::new(runtime),
        )
    }

    pub(crate) fn can_start_node(&self) -> bool {
        self.node.is_none() && self.stopped.load(Ordering::SeqCst)
    }

    pub(crate) fn can_stop_node(&self) -> bool {
        self.node.is_some() && self.started.load(Ordering::SeqCst)
    }

    pub(crate) fn start_beta_node(&mut self) {
        let published = self.published.clone();
        let callbacks = NodeCallbacks::builder()
            .on_publish(move |_channel_id, _message| {
                published.fetch_add(1, Ordering::SeqCst);
            })
            .finish();
        let node = self
            .node_builder_factory
            .builder_for(Networks::NanoBetaNetwork)
            .callbacks(callbacks)
            .finish()
            .unwrap();
        let node = Arc::new(node);
        let node_l = node.clone();
        let started = self.started.clone();
        let stopped = self.stopped.clone();
        self.runtime.spawn_blocking(move || {
            node_l.start();
            started.store(true, Ordering::SeqCst);
            stopped.store(false, Ordering::SeqCst);
        });

        self.node = Some(node);
    }

    pub(crate) fn stop_node(&mut self) {
        if let Some(node) = self.node.take() {
            self.started.store(false, Ordering::SeqCst);
            {
                let stopped = self.stopped.clone();
                self.runtime.spawn_blocking(move || {
                    node.stop();
                    stopped.store(true, Ordering::SeqCst);
                });
            }
        }
    }

    pub(crate) fn status(&self) -> &'static str {
        if self.node.is_some() {
            if self.started.load(Ordering::SeqCst) {
                "running"
            } else {
                "starting..."
            }
        } else {
            if !self.stopped.load(Ordering::SeqCst) {
                "stopping..."
            } else {
                "not running"
            }
        }
    }

    pub(crate) fn messages_published(&self) -> String {
        self.published
            .load(Ordering::SeqCst)
            .to_formatted_string(&Locale::en)
    }
}

impl Default for AppModel {
    fn default() -> Self {
        Self::new(
            Arc::new(NullableRuntime::default()),
            NodeBuilderFactory::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initial_status() {
        let model = AppModel::new(
            Arc::new(NullableRuntime::new_null()),
            NodeBuilderFactory::new_null(),
        );
        assert_eq!(model.can_start_node(), true);
        assert_eq!(model.can_stop_node(), false);
        assert_eq!(model.status(), "not running");
        assert_eq!(model.messages_published(), "0");
    }

    #[tokio::test]
    async fn starting_node() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppModel::new(runtime.clone(), NodeBuilderFactory::new_null());

        model.start_beta_node();

        assert_eq!(model.can_start_node(), false);
        assert_eq!(model.can_stop_node(), false);
        assert_eq!(model.status(), "starting...");
        assert_eq!(runtime.blocking_spawns(), 1);
    }

    #[tokio::test]
    async fn starting_completed() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppModel::new(runtime.clone(), NodeBuilderFactory::new_null());
        model.start_beta_node();

        runtime.run_nulled_blocking_task();

        assert_eq!(model.status(), "running");
        assert_eq!(model.can_start_node(), false);
        assert_eq!(model.can_stop_node(), true);
    }

    #[tokio::test]
    async fn stopping_node() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppModel::new(runtime.clone(), NodeBuilderFactory::new_null());
        model.start_beta_node();
        runtime.run_nulled_blocking_task();
        model.stop_node();
        assert_eq!(model.can_start_node(), false);
        assert_eq!(model.can_stop_node(), false);
        assert_eq!(model.status(), "stopping...");
        assert_eq!(runtime.blocking_spawns(), 2);
    }

    #[tokio::test]
    async fn stopping_completed() {
        let runtime = Arc::new(NullableRuntime::new_null());
        let mut model = AppModel::new(runtime.clone(), NodeBuilderFactory::new_null());
        model.start_beta_node();
        runtime.run_nulled_blocking_task();
        model.stop_node();
        runtime.run_nulled_blocking_task();
        assert_eq!(model.can_start_node(), true);
        assert_eq!(model.can_stop_node(), false);
        assert_eq!(model.status(), "not running");
    }
}
