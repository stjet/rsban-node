use eframe::egui::{self, global_theme_preference_buttons};
use rsnano_core::{work::WorkPoolImpl, Networks};
use rsnano_node::{
    config::NodeConfig, working_path_for, NetworkParams, Node, NodeBuilder, NodeExt,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::runtime::Runtime;

fn main() -> eframe::Result {
    let runtime = Runtime::new().unwrap();
    let model = AppModel::new(runtime.handle().clone());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RsNano Insight",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_pixels_per_point(1.20);
            Ok(Box::new(InsightApp::new(model)))
        }),
    )
}

struct InsightApp {
    model: AppModel,
}

impl InsightApp {
    fn new(model: AppModel) -> Self {
        Self { model }
    }
}

impl eframe::App for InsightApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            global_theme_preference_buttons(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.model.can_start_node() {
                if ui.button("Start beta node").clicked() {
                    self.model.start_beta_node();
                }
            }

            if self.model.can_stop_node() {
                if ui.button("Stop node").clicked() {
                    self.model.stop_node();
                }
            }

            ui.horizontal(|ui| {
                ui.label("Status: ");
                ui.label(self.model.status());
            });
            ui.horizontal(|ui| {
                ui.label("Messages published: ");
                ui.label(self.model.messages_published().to_string());
            });
        });
        ctx.request_repaint();
    }
}

pub(crate) struct AppModel {
    runtime: tokio::runtime::Handle,
    node: Option<Arc<Node>>,
    started: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    published: Arc<AtomicUsize>,
}

impl AppModel {
    pub(crate) fn new(runtime: tokio::runtime::Handle) -> Self {
        Self {
            runtime,
            node: None,
            started: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(true)),
            published: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn can_start_node(&self) -> bool {
        self.node.is_none() && self.stopped.load(Ordering::SeqCst)
    }

    pub(crate) fn can_stop_node(&self) -> bool {
        self.node.is_some() && self.started.load(Ordering::SeqCst)
    }

    pub(crate) fn start_beta_node(&mut self) {
        let network = Networks::NanoBetaNetwork;
        let network_params = NetworkParams::new(network);
        let node_config = NodeConfig::new(None, &network_params, 2);
        let work = Arc::new(WorkPoolImpl::new(
            network_params.work.clone(),
            node_config.work_threads as usize,
            Duration::from_nanos(node_config.pow_sleep_interval_ns as u64),
        ));

        let node_path = working_path_for(network).unwrap();

        let published = self.published.clone();
        let node = NodeBuilder::new(Networks::NanoBetaNetwork)
            .runtime(self.runtime.clone())
            .data_path(node_path)
            .config(node_config)
            .network_params(network_params)
            .work(work)
            .on_publish(Arc::new(move |_channel_id, _message| {
                published.fetch_add(1, Ordering::SeqCst);
            }))
            .finish();
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
                "stopping"
            } else {
                "not running"
            }
        }
    }

    pub(crate) fn messages_published(&self) -> usize {
        self.published.load(Ordering::SeqCst)
    }
}
