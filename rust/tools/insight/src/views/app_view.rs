use super::{
    show_peers, LedgerStatsView, MessageRecorderControlsView, MessageStatsView, MessageTabView,
    NodeRunnerView, TabBarView,
};
use crate::view_models::{AppViewModel, Tab};
use eframe::egui::{
    self, global_theme_preference_switch, CentralPanel, Grid, ProgressBar, TopBottomPanel,
};
use rsnano_node::{cementation::ConfirmingSetInfo, consensus::ActiveElectionsInfo};

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
            Tab::Queues => show_queues(ctx, &self.model.aec_info, &self.model.confirming_set),
        }

        // Repaint to show the continuously increasing current block and message counters
        ctx.request_repaint();
    }
}

fn show_queues(ctx: &egui::Context, info: &ActiveElectionsInfo, confirming: &ConfirmingSetInfo) {
    CentralPanel::default().show(ctx, |ui| {
        ui.heading("Active Elections");
        Grid::new("aec_grid").num_columns(2).show(ui, |ui| {
            ui.label("total");
            ui.add(
                ProgressBar::new(info.total as f32 / info.max_queue as f32)
                    .text(info.total.to_string())
                    .desired_width(300.0),
            );
            ui.end_row();

            ui.label("priority");
            ui.add(
                ProgressBar::new(info.priority as f32 / info.max_queue as f32)
                    .text(info.priority.to_string())
                    .desired_width(300.0),
            );
            ui.end_row();

            ui.label("hinted");
            ui.add(
                ProgressBar::new(info.hinted as f32 / info.max_queue as f32)
                    .text(info.hinted.to_string())
                    .desired_width(300.0),
            );
            ui.end_row();

            ui.label("optimistic");
            ui.add(
                ProgressBar::new(info.optimistic as f32 / info.max_queue as f32)
                    .text(info.optimistic.to_string())
                    .desired_width(300.0),
            );
            ui.end_row();
        });

        ui.heading("Miscellaneous");
        Grid::new("misc_grid").num_columns(2).show(ui, |ui| {
            ui.label("confirming");
            ui.add(
                ProgressBar::new(confirming.size as f32 / confirming.max_size as f32)
                    .text(confirming.size.to_string())
                    .desired_width(300.0),
            );
            ui.end_row();
        });
    });
}
