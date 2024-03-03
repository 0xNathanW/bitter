use std::{collections::HashMap, io::{stdout, Stdout}};
use bittorrent::{ClientHandle, MetaInfo, UserRx, CommandToUser};
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, layout::Layout, widgets::{Block, Borders, Paragraph}, Frame};
use anyhow::Result;
use ratatui_explorer::FileExplorer;

use crate::{event::spawn_events, torrents::TorrentTable};

pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct App {
    
    handle:   ClientHandle,
    
    user_rx:  UserRx,
    
    file_explorer: FileExplorer,

    torrent_table: TorrentTable,

    is_file_explorer_open: bool,

    quit: bool,

}

impl App {

    pub fn new() -> Result<Self> {
        let (handle, user_rx) = bittorrent::start_client(None)?;
        Ok(Self {
            handle,
            user_rx,
            torrent_table: TorrentTable::new(),
            file_explorer: FileExplorer::new()?,
            is_file_explorer_open: false,
            quit: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {

        let mut terminal = ratatui::Terminal::new(CrosstermBackend::new(stdout()))?;
        let (event_handle, mut event_rx) = spawn_events();

        // Draw the initial frame.
        terminal.draw(|f| {
            self.view(f);
        })?;

        loop {
            tokio::select! {
                
                // Some event from our bittorrent client
                Some(client_event) = self.user_rx.recv() => {
                    match client_event {
                        CommandToUser::TorrentError(err) => {
                            // TODO: popup probs.
                        },

                        CommandToUser::TorrentComplete(id) => {
                            self.torrent_table.remove_torrent(id);
                        },

                        CommandToUser::TorrentStats { id, stats } => {
                            self.torrent_table.update_torrent(id, stats);
                        }
                    }
                },

                Some(event) = event_rx.recv() => self.handle_event(event),
            }

            terminal.draw(|f| {
                if self.is_file_explorer_open {
                    f.render_widget_ref(self.file_explorer.widget(), f.size());
                } else {
                    self.view(f);
                }
            })?;

            if self.quit {
                break;
            }
        }

        Ok(())
    }

    fn view(&mut self, f: &mut Frame) {

        // ratatui Layout with 2 columns 2 rows equal size
        let rows = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(
                [
                    ratatui::layout::Constraint::Percentage(50),
                    ratatui::layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(f.size());

        let top_row = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(
                [
                    ratatui::layout::Constraint::Percentage(50),
                    ratatui::layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(rows[0]);

        let bottom_row = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(
                [
                    ratatui::layout::Constraint::Percentage(50),
                    ratatui::layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(rows[1]);


        // f.render_widget(self.torrent_table, top_row[0]);
        self.torrent_table.render(f, top_row[0]);

        // Right column
        let tr = Paragraph::new("Right column")
            .block(Block::default().title("Right").borders(Borders::ALL));
        f.render_widget(tr, top_row[1]);

        // // Bottom row
        let bl = Paragraph::new("boo")
            .block(Block::default().title("Bottom").borders(Borders::ALL));
        f.render_widget(bl, bottom_row[0]);

        // // Bottom row
        let br = Paragraph::new("Bottom Left")
            .block(Block::default().title("Bottom").borders(Borders::ALL));
        f.render_widget(br, bottom_row[1]);
    }

    // TODO: handle pasting a magnet link
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                if key.kind != event::KeyEventKind::Press {
                    return;    
                }
                if self.is_file_explorer_open {
                    match key.code {
                        event::KeyCode::Esc => {
                            self.is_file_explorer_open = false;
                        },
                        event::KeyCode::Char('q') => {
                            self.quit = true;
                        },
                        event::KeyCode::Enter => {
                            let file = self.file_explorer.current();
                            if file.is_dir() {
                                self.file_explorer.handle(&event).ok();
                            } else {
                                let metainfo = MetaInfo::new(file.path()).unwrap();
                                self.handle.new_torrent(metainfo.clone(), None).ok();
                                self.torrent_table.add_torrent(metainfo);
                                self.is_file_explorer_open = false;
                            }
                        }

                        _ => {
                            self.file_explorer.handle(&event).ok();
                        }
                    }
                } else {
                    match key.code {
                        event::KeyCode::Char('q') => {
                            self.quit = true;
                        },
                        event::KeyCode::Char('n') => {
                            self.is_file_explorer_open = true;
                        },
                        _ => {}
                    }
                }
                

            },
            _ => {}
        }
    }
}

