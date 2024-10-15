use crate::view_models::MessageStatsViewModel;
use eframe::egui::Ui;

pub(crate) struct MessageStatsView<'a>(MessageStatsViewModel<'a>);

impl<'a> MessageStatsView<'a> {
    pub fn new(model: MessageStatsViewModel<'a>) -> Self {
        Self(model)
    }

    pub fn view(&self, ui: &mut Ui) {
        ui.label("Messages:");
        ui.label(self.0.messages_sent());
        ui.label("sent");
        ui.add_space(10.0);
        ui.label(self.0.messages_received());
        ui.label("received");
    }
}
