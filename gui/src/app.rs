use eframe::Frame;
use egui::Ui;


pub struct App {
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
        }
    }
}

impl eframe::App for App {

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::trace!(ui);
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                self.top_bar_content(ui, frame);
            });
        });

    }
}

impl App {

    fn top_bar_content(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        // For toggling light/dark mode.
        egui::widgets::global_dark_light_mode_switch(ui);
        
        ui.separator();
    }

}