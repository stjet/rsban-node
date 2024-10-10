use crate::AppModel;
use eframe::egui::{self, global_theme_preference_switch, Button, Sense};
use egui_extras::{Column, TableBuilder};

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
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(1.0);
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
            ui.add_space(1.0);
        });
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                global_theme_preference_switch(ui);
                ui.separator();
                ui.label("Messages:");
                ui.label(self.model.messages_sent());
                ui.label("sent");
                ui.add_space(10.0);
                ui.label(self.model.messages_received());
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
            TableBuilder::new(ui)
                .striped(true)
                .sense(Sense::click())
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder())
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Channel");
                    });
                    header.col(|ui| {
                        ui.strong("Direction");
                    });
                    header.col(|ui| {
                        ui.strong("Message");
                    });
                })
                .body(|body| {
                    body.rows(20.0, self.model.message_count(), |mut row| {
                        let row_model = self.model.get_row(row.index());
                        row.col(|ui| {
                            ui.label(row_model.channel_id);
                        });
                        row.col(|ui| {
                            ui.label(row_model.direction);
                        });
                        row.col(|ui| {
                            ui.label(row_model.message);
                        });
                    })
                });
        });
        ctx.request_repaint();
    }
}
