use crate::view_models::ChannelsViewModel;
use eframe::egui::{self, CentralPanel, Label, Sense, Ui};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};

pub(crate) fn show_peers(ctx: &egui::Context, model: ChannelsViewModel) {
    CentralPanel::default().show(ctx, |ui| {
        if model.channel_count() == 0 {
            show_no_connected_peers(ui);
        } else {
            show_peers_table(ui, model);
        }
    });
}

fn show_no_connected_peers(ui: &mut Ui) {
    StripBuilder::new(ui)
        .size(Size::remainder())
        .size(Size::exact(50.0))
        .size(Size::remainder())
        .vertical(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.vertical_centered_justified(|ui| ui.heading("not connected to any peers"));
            });
            strip.empty();
        });
}

fn show_peers_table(ui: &mut Ui, mut model: ChannelsViewModel) {
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .auto_shrink(false)
        .sense(Sense::click())
        .column(Column::auto())
        .column(Column::auto())
        .column(Column::remainder())
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Channel");
            });
            header.col(|ui| {
                ui.strong("in/out");
            });
            header.col(|ui| {
                ui.strong("Remote Addr");
            });
        })
        .body(|body| {
            body.rows(20.0, model.channel_count(), |mut row| {
                let Some(row_model) = model.get_row(row.index()) else {
                    return;
                };
                if row_model.is_selected {
                    row.set_selected(true);
                }
                row.col(|ui| {
                    ui.add(Label::new(row_model.channel_id).selectable(false));
                });
                row.col(|ui| {
                    ui.add(Label::new(row_model.direction).selectable(false));
                });
                row.col(|ui| {
                    ui.add(Label::new(row_model.remote_addr).selectable(false));
                });
                if row.response().clicked() {
                    model.select(row.index());
                }
            })
        });
}
