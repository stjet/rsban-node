use crate::AppModel;
use eframe::egui::{self, global_theme_preference_switch, Button};

pub(crate) struct InsightApp {
    model: AppModel,
}

impl InsightApp {
    pub(crate) fn new(model: AppModel) -> Self {
        Self { model }
    }
}

impl eframe::App for InsightApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                global_theme_preference_switch(ui);
                ui.separator();
                ui.label("Messages:");
                ui.label(self.model.messages_published());
                ui.label("sent");
                ui.add_space(10.0);
                ui.label("0");
                ui.label("received");
                ui.separator();
                ui.label("Blocks:");
                ui.label("0");
                ui.label("bps");
                ui.add_space(10.0);
                ui.label("0");
                ui.label("cps");
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.model.can_start_node(), Button::new("Start beta node"))
                    .clicked()
                {
                    self.model.start_beta_node();
                }

                if ui
                    .add_enabled(self.model.can_stop_node(), Button::new("Stop node"))
                    .clicked()
                {
                    self.model.stop_node();
                }
                ui.label(self.model.status());
            });

            ui.separator();
        });
        ctx.request_repaint();
    }
}
