use super::{
    show_peers, LedgerStatsView, MessageRecorderControlsView, MessageStatsView, MessageTabView,
    NodeRunnerView, TabBarView,
};
use crate::view_models::{AppViewModel, Tab};
use eframe::egui::{
    self, global_theme_preference_switch, CentralPanel, Grid, ProgressBar, TopBottomPanel,
};
use rsnano_node::{block_processing::BlockProcessorInfo, consensus::VoteProcessorInfo,
    cementation::ConfirmingSetInfo, consensus::ActiveElectionsInfo};

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
            Tab::Queues => show_queues(ctx, &self.model.aec_info, 
                                            &self.model.confirming_set, 
                                            &self.model.block_processor_info,
                                            &self.model.vote_processor_info
                                        ),
        }

        // Repaint to show the continuously increasing current block and message counters
        ctx.request_repaint();
    }
}

fn show_queues(ctx: &egui::Context, 
                info: &ActiveElectionsInfo, 
                confirming: &ConfirmingSetInfo,
                block_processor_info: &BlockProcessorInfo,
                vote_processor_info: &VoteProcessorInfo,
               ) {
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

        ui.heading("Block Processor Queues");
        Grid::new("block_processor_queues_grid")
            .num_columns(2)
            .show(ui, |ui| {
                for queue_info in &block_processor_info.queues {
                    ui.label(format!("{:?}", queue_info.source));
                    ui.add(
                        ProgressBar::new(queue_info.size as f32 / queue_info.max_size as f32)
                            .text(format!("{}/{}", queue_info.size, queue_info.max_size))
                            .desired_width(300.0),
                    );
                    ui.end_row();
                }
                ui.label("Total");
                ui.add(
                    ProgressBar::new(
                        block_processor_info.total_size as f32
                            / block_processor_info
                                .queues
                                .iter()
                                .map(|q| q.max_size)
                                .sum::<usize>() as f32,
                    )
                    .text(block_processor_info.total_size.to_string())
                    .desired_width(300.0),
                );
                ui.end_row();
            });
        
        ui.heading("Vote Processor Queues"); // New section
        Grid::new("vote_processor_queues_grid")
            .num_columns(2)
            .show(ui, |ui| {
                for queue_info in &vote_processor_info.queues {
                    ui.label(format!("{:?}", queue_info.source));
                    let progress = if queue_info.max_size > 0 {
                        queue_info.size as f32 / queue_info.max_size as f32
                    } else {
                        0.0
                    };
                    ui.add(
                        ProgressBar::new(progress)
                            .text(format!("{}/{}", queue_info.size, queue_info.max_size))
                            .desired_width(300.0),
                    );
                    ui.end_row();
                }
                ui.label("Total");
                let total_max_size: usize = vote_processor_info
                    .queues
                    .iter()
                    .map(|q| q.max_size)
                    .sum();
                let total_progress = if total_max_size > 0 {
                    vote_processor_info.total_size as f32 / total_max_size as f32
                } else {
                    0.0
                };
                ui.add(
                    ProgressBar::new(total_progress)
                        .text(vote_processor_info.total_size.to_string())
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
