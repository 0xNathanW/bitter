use std::rc::Rc;
use core::torrent::Torrent;
use egui::CollapsingHeader;

use super::Widget;

pub struct TorrentInfo(pub Rc<Torrent>);

impl Widget for TorrentInfo {
    
    fn name(&self) -> &'static str {
        "Torrent Info"
    }

    fn display(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .open(open)
            .vscroll(true)
            .show(ctx, |ui| {
                
                CollapsingHeader::new("Announce")
                    .show(ui, |ui| {
                        ui.label(self.0.announce())
                    });
                
                CollapsingHeader::new("Announce List")
                    .show(ui, |ui| {
                        if let Some(list) = self.0.announce_list() {
                            for (i, announce) in list.iter().enumerate() {
                                ui.label(format!("#{}: {}", i, announce));
                            }
                        } else {
                            ui.label("None");
                        }
                    });

                CollapsingHeader::new("Creator")
                    .show(ui, |ui| {
                        if let Some(creator) = self.0.created_by() {
                            ui.label(creator);
                        } else {
                            ui.label("None");
                        }
                    });
                
                CollapsingHeader::new("Creation Date")
                    .show(ui, |ui| {
                        if let Some(date) = self.0.creation_date_fmt() {
                            ui.label(date);
                        } else {
                            ui.label("None");
                        }
                    });
                    
                CollapsingHeader::new("Comment")
                    .show(ui, |ui| {
                        if let Some(comment) = self.0.comment() {
                            ui.label(comment);
                        } else {
                            ui.label("None");
                        }
                    });

                CollapsingHeader::new("Encoding")
                    .show(ui, |ui| {
                        if let Some(encoding) = self.0.encoding() {
                            ui.label(encoding);
                        } else {
                            ui.label("None");
                        }
                    });

                CollapsingHeader::new("Info Hash")
                    .show(ui, |ui| {
                        ui.label(format!("0x{}" ,hex::encode(self.0.info_hash())));
                    }).header_response
                        .on_hover_text("The info hash is the SHA1 hash of the bencoded form of the info dictionary.");
                
                CollapsingHeader::new("Info")
                    .show(ui, |ui| {
                    
                        CollapsingHeader::new("Name")
                            .show(ui, |ui| {
                                ui.label(self.0.name());
                            });
                        
                        CollapsingHeader::new("Piece Length")
                        .show(ui, |ui| {
                            ui.label(format!("{} bytes", self.0.piece_length().to_string()));
                        });
                    
                        CollapsingHeader::new("Pieces")
                            .show(ui, |ui| {
                                let width = self.0.num_pieces().to_string().len() + 1;

                                egui::containers::ScrollArea::new([false, true]).show(ui, |ui| {
                                    for (i, piece) in self.0.pieces_iter().enumerate() {
                                        ui.label(format!("{i:width$} : 0x{}", hex::encode(piece)));
                                    }
                                });
                            }).header_response
                                .on_hover_text("The SHA1 hash of each piece in the torrent.");
                        
                        CollapsingHeader::new("Private")
                            .show(ui, |ui| {
                                ui.label(self.0.is_private().to_string());
                            });
                        
                        CollapsingHeader::new("Size")
                            .show(ui, |ui| {
                                ui.label(self.0.size_fmt().to_string());
                            });
                            
                        // If single file.
                        if !self.0.is_multi_file() {

                            CollapsingHeader::new("md5sum")
                                .show(ui, |ui| {
                                    if let Some(md5) = self.0.md5sum() {
                                        ui.label(hex::encode(md5));
                                    } else {
                                        ui.label("None");
                                    }
                                });
                        
                        } else {
                            
                            if let Some(files) = self.0.files() {
                                for file in files {
                                    CollapsingHeader::new(format!("{}/{}", file.path(), self.0.name()))
                                        .show(ui, |ui| {

                                            CollapsingHeader::new("Size")
                                                .show(ui, |ui| {
                                                    ui.label(file.size_fmt());
                                                });

                                            CollapsingHeader::new("md5sum")
                                                .show(ui, |ui| {
                                                    if let Some(md5) = file.md5sum() {
                                                        ui.label(hex::encode(md5));
                                                    } else {
                                                        ui.label("None");
                                                    }
                                                });

                                        });
                                }
                            }  

                        }
                    });
            });
    }
}