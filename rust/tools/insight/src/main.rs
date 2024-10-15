mod message_recorder;
mod node_factory;
mod node_runner;
mod nullable_runtime;
mod view_models;
mod views;

use eframe::egui;
use tokio::runtime::Runtime;
use view_models::AppViewModel;
use views::AppView;

fn main() -> eframe::Result {
    let runtime = Runtime::new().unwrap();
    let model = AppViewModel::with_runtime(runtime.handle().clone());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RsNano Insight",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_zoom_factor(1.15);
            Ok(Box::new(AppView::new(model)))
        }),
    )
}
