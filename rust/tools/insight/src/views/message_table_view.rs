use crate::view_models::MessageTableViewModel;
use eframe::egui::{Label, Sense, Ui};
use egui_extras::{Column, TableBuilder};

pub(crate) struct MessageTableView<'a> {
    model: &'a mut MessageTableViewModel,
}

impl<'a> MessageTableView<'a> {
    pub(crate) fn new(model: &'a mut MessageTableViewModel) -> Self {
        Self { model }
    }

    pub(crate) fn show(&mut self, ui: &mut Ui) {
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
                    ui.strong("Direction");
                });
                header.col(|ui| {
                    ui.strong("Message");
                });
            })
            .body(|body| {
                body.rows(20.0, self.model.message_count(), |mut row| {
                    let Some(row_model) = self.model.get_row(row.index()) else {
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
                        ui.add(Label::new(row_model.message).selectable(false));
                    });
                    if row.response().clicked() {
                        self.model.select_message(row.index());
                    }
                })
            });
    }
}
