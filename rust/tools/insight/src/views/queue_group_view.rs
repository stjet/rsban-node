use crate::view_models::QueueGroupViewModel;
use eframe::egui::{Align, Layout, ProgressBar, Ui};
use egui_extras::{Size, StripBuilder};

pub(crate) fn show_queue_group(ui: &mut Ui, model: QueueGroupViewModel) {
    ui.heading(model.heading);

    for queue in model.queues {
        ui.horizontal(|ui| {
            StripBuilder::new(ui)
                .size(Size::exact(100.0))
                .size(Size::exact(300.0))
                .size(Size::remainder())
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(queue.label);
                        });
                    });
                    strip.cell(|ui| {
                        ui.add(ProgressBar::new(queue.progress).text(queue.value));
                    });
                    strip.cell(|ui| {
                        ui.label(queue.max);
                    });
                });
        });
    }
}
