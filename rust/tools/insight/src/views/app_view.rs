use super::{
    LedgerStatsView, MessageRecorderControlsView, MessageStatsView, MessageTableView, MessageView,
    NodeRunnerView,
};
use crate::AppViewModel;
use eframe::egui::{self, global_theme_preference_switch, CentralPanel, SidePanel, TopBottomPanel};

pub(crate) struct AppView {
    model: AppViewModel,
}

impl AppView {
    pub(crate) fn new(model: AppViewModel) -> Self {
        Self { model }
    }
}

impl AppView {
    fn show_top_panel(&mut self, ctx: &egui::Context) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(1.0);
            ui.horizontal(|ui| {
                NodeRunnerView::new(&mut self.model.node_runner).show(ui);
                ui.separator();
                MessageRecorderControlsView::new(&self.model.msg_recorder).show(ui);
            });
            ui.add_space(1.0);
        });
    }

    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
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

    fn show_message_overview_panel(&mut self, ctx: &egui::Context) {
        SidePanel::left("overview_panel")
            .default_width(300.0)
            .min_width(300.0)
            .resizable(true)
            .show(ctx, |ui| {
                MessageTableView::new(&mut self.model.message_table).show(ui);
            });
    }

    fn show_message_details_panel(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            if let Some(details) = self.model.message_table.selected_message() {
                MessageView::new(&details).show(ui);
            }
        });
    }
}

impl eframe::App for AppView {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.model.update();
        self.show_top_panel(ctx);
        self.show_bottom_panel(ctx);
        self.show_message_overview_panel(ctx);
        self.show_message_details_panel(ctx);
        // Repaint to show the continuously increasing current block and message counters
        ctx.request_repaint();
    }
}