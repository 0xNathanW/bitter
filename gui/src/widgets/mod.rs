use std::collections::BTreeSet;

#[derive(Default)]
pub struct Widgets {
    // Widgets.
    items: Vec<Box<dyn Widget>>,
    // Tracks which widgets are open.
    open:  BTreeSet<String>,
}

pub trait Widget {
    
    fn name(&self) -> &'static str;

    fn display(&mut self, ctx: &egui::Context, open: &mut bool); 
}

impl Widgets {

    pub fn new() -> Self {
        Self {
            
            items: vec![

            ],

            open: BTreeSet::new(),
        }
    }

    pub fn display(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        
        egui::SidePanel::left("widgets")
            .resizable(true)
            .show(ctx, |ui| {

                egui::widgets::global_dark_light_mode_buttons(ui);
                ui.add_space(10.0);

                ui.vertical_centered(|ui| {
                    ui.heading("Widgets");
                    ui.separator();
                });

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        let Self { items, open } = self;

                        for item in items {
                            let mut is_open = open.contains(item.name());
                            ui.toggle_value(&mut is_open, item.name());
                            set_open(open, item.name(), is_open);
                        }

                    });
                
                });

            });
    }

    fn add_contents(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {

    }

}

// Add/remove widget from opened set.
fn set_open(btree: &mut BTreeSet<String>, key: &'static str, is_open: bool) {
    if is_open {
        if !btree.contains(key) {
            btree.insert(key.to_owned());
        }
    } else {
        btree.remove(key);
    }
}