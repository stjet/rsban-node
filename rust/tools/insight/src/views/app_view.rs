use super::{
    queue_group_view::show_queue_group, show_peers, LedgerStatsView, MessageRecorderControlsView,
    MessageStatsView, MessageTabView, NodeRunnerView, TabBarView,
};
use crate::view_models::{
    create_queue_group_view_model, AppViewModel, QueueGroupViewModel, QueueViewModel, Tab,
};
use eframe::egui::{
    self, global_theme_preference_switch, warn_if_debug_build, CentralPanel, Grid, ProgressBar,
    TopBottomPanel,
};
use num_format::{Locale, ToFormattedString};
use rsnano_node::{
    block_processing::BlockSource,
    cementation::ConfirmingSetInfo,
    consensus::{ActiveElectionsInfo, RepTier},
    transport::{FairQueueInfo, QueueInfo},
};
use strum::IntoEnumIterator;

pub(crate) struct AppView {
    model: AppViewModel,
}

impl AppView {
    pub(crate) fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        let model = AppViewModel::with_runtime(runtime_handle);
        Self { model }
    }
}

impl AppView {
    fn show_node_runner(&mut self, ctx: &egui::Context) {
        TopBottomPanel::top("node_runner_panel").show(ctx, |ui| {
            ui.add_space(1.0);
            ui.horizontal(|ui| {
                NodeRunnerView::new(&mut self.model.node_runner).show(ui);
                ui.separator();
                MessageRecorderControlsView::new(&self.model.msg_recorder).show(ui);
            });
            ui.add_space(1.0);
        });
    }

    fn show_tabs(&mut self, ctx: &egui::Context) {
        TopBottomPanel::top("tabs_panel").show(ctx, |ui| {
            TabBarView::new(&mut self.model.tabs).show(ui);
        });
    }

    fn show_stats(&mut self, ctx: &egui::Context) {
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                global_theme_preference_switch(ui);
                ui.separator();
                MessageStatsView::new(self.model.message_stats()).view(ui);
                ui.separator();
                LedgerStatsView::new(self.model.ledger_stats()).view(ui);
                warn_if_debug_build(ui);
            });
        });
    }
}

impl eframe::App for AppView {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.model.update();
        self.show_node_runner(ctx);
        self.show_tabs(ctx);
        self.show_stats(ctx);

        match self.model.tabs.selected_tab() {
            Tab::Peers => show_peers(ctx, self.model.channels()),
            Tab::Messages => MessageTabView::new(&mut self.model).show(ctx),
            Tab::Queues => show_queues(
                ctx,
                &self.model.aec_info,
                &self.model.confirming_set,
                &self.model.block_processor_info,
                &self.model.vote_processor_info,
            ),
        }

        // Repaint to show the continuously increasing current block and message counters
        ctx.request_repaint();
    }
}

fn show_queues(
    ctx: &egui::Context,
    info: &ActiveElectionsInfo,
    confirming: &ConfirmingSetInfo,
    block_processor_info: &FairQueueInfo<BlockSource>,
    vote_processor_info: &FairQueueInfo<RepTier>,
) {
    CentralPanel::default().show(ctx, |ui| {
        let group = QueueGroupViewModel {
            heading: "Active Elections".to_string(),
            queues: vec![
                QueueViewModel {
                    label: "Priority".to_string(),
                    value: info.priority.to_formatted_string(&Locale::en),
                    max: info.max_queue.to_formatted_string(&Locale::en),
                    progress: info.priority as f32 / info.max_queue as f32,
                },
                QueueViewModel {
                    label: "Hinted".to_string(),
                    value: info.hinted.to_formatted_string(&Locale::en),
                    max: info.max_queue.to_formatted_string(&Locale::en),
                    progress: info.hinted as f32 / info.max_queue as f32,
                },
                QueueViewModel {
                    label: "Optimistic".to_string(),
                    value: info.optimistic.to_formatted_string(&Locale::en),
                    max: info.max_queue.to_formatted_string(&Locale::en),
                    progress: info.optimistic as f32 / info.max_queue as f32,
                },
                QueueViewModel {
                    label: "Total".to_string(),
                    value: info.total.to_formatted_string(&Locale::en),
                    max: info.max_queue.to_formatted_string(&Locale::en),
                    progress: info.total as f32 / info.max_queue as f32,
                },
            ],
        };
        show_queue_group(ui, group);

        ui.add_space(10.0);

        let group = create_queue_group_view_model("Block Processor", block_processor_info);
        show_queue_group(ui, group);

        ui.add_space(10.0);

        let group = create_queue_group_view_model("Vote Processor", vote_processor_info);
        show_queue_group(ui, group);

        ui.add_space(10.0);

        let group = QueueGroupViewModel {
            heading: "Miscellaneous".to_string(),
            queues: vec![QueueViewModel {
                label: "Confirming".to_string(),
                value: confirming.size.to_formatted_string(&Locale::en),
                max: confirming.max_size.to_formatted_string(&Locale::en),
                progress: confirming.size as f32 / confirming.max_size as f32,
            }],
        };
        show_queue_group(ui, group);
    });
}
