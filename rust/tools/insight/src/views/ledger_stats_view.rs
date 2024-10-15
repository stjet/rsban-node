use crate::view_models::LedgerStatsViewModel;
use eframe::egui::Ui;

pub(crate) struct LedgerStatsView<'a>(LedgerStatsViewModel<'a>);

impl<'a> LedgerStatsView<'a> {
    pub fn new(model: LedgerStatsViewModel<'a>) -> Self {
        Self(model)
    }

    pub fn view(&self, ui: &mut Ui) {
        ui.label("Blocks:");
        ui.label("?");
        ui.label("bps");
        ui.add_space(10.0);
        ui.label("?");
        ui.label("cps");
        ui.add_space(10.0);
        ui.label(self.0.block_count());
        ui.label("blocks");
        ui.add_space(10.0);
        ui.label(self.0.cemented_count());
        ui.label("cemented");
    }
}
