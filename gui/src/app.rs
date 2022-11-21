use eframe::Frame;
use egui::Ui;

use crate::widgets::Widgets;

pub struct App {
    widgets: Widgets,
    torrent_loaded: bool,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self { widgets: Widgets::new(), torrent_loaded: false }
    }
}

impl eframe::App for App {

    // Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {

        if !self.torrent_loaded {
            self.file_input(ctx);
            return;
        }
        
        self.widgets.display(ctx, frame);

    }
}

impl App {

    fn file_input(&mut self, ctx: &egui::Context) {

        use std::fmt::Write as _;
        use egui::layers;

        // Paint text to middle of the screen.
        let painter = ctx.layer_painter(
            layers::LayerId::new(layers::Order::Foreground, egui::Id::new("file_drop"))
        );

        // Preview.
        let msg = if !ctx.input().raw.hovered_files.is_empty() {

            let mut base = "Dropping file:\n".to_string();
            let file = &ctx.input().raw.hovered_files[0];

            if let Some(path) = &file.path {
                write!(base, "\n{}", path.display()).ok();
                if path.extension() != Some(std::ffi::OsStr::new("torrent")) {
                    base += "\n\nInvalid: this is not a torrent file";
                }
            } else if !file.mime.is_empty() {
                write!(base, "\n{}", file.mime).ok();
            } else {
                base += "Invalid: Unreadable file";
            }
            base
        } 
        // Prompt for file.
        else {
            "Drag and drop a torrent file".to_string()
        };

        let screen_rect = ctx.input().screen_rect();
            painter.rect_filled(screen_rect, 0.0, egui::color::Color32::from_black_alpha(192));
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                msg,
                egui::TextStyle::Heading.resolve(&ctx.style()),
                egui::color::Color32::WHITE,
            );

        if !ctx.input().raw.dropped_files.is_empty() {
            let file = ctx.input().raw.dropped_files[0].clone();
            if let Some(path) = &file.path {
                if path.extension() == Some(std::ffi::OsStr::new("torrent")) {
                    self.torrent_loaded = true;
                    self.widgets.load_torrent(path);
                }
            }
        }
    }    


}