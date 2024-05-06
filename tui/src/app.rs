use std::{collections::HashMap, io::{stdout, Stdout}};
use bittorrent::{ClientHandle, UserCommand, MetaInfo, TorrentID, UserRx};
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, layout::Layout, prelude::*, widgets::{self, Clear}, Frame};
use color_eyre::Result;
use crate::{event::spawn_events, data::TorrentData, ui::{self, UiStates}};

pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct App {
    
    // Handle to the bittorrent client.
    handle:   ClientHandle,
    
    // Channel to recieve messages from the bittorrent client.
    user_rx:  UserRx,
    
    // List of active torrents.
    torrents: Vec<TorrentData>,

    // Maps torrent id to index in the torrents vector.
    torrent_lookup: HashMap<TorrentID, usize>,

    // Ui stateful elements.
    ui: UiStates,

    // Index of the currently selected torrent.
    selected_idx: usize,

    // Flag to quit the app on next loop.
    quit: bool,

}

impl App {

    pub fn new() -> Result<Self> {
        // Run the bittorrent client in a separate task.
        let (handle, user_rx) = bittorrent::start_client(None);
        Ok(Self {
            handle,
            user_rx,
            torrents: Vec::new(),
            torrent_lookup: HashMap::new(),
            ui: UiStates::new()?,
            selected_idx: 0,
            quit: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {

        let mut terminal = ratatui::Terminal::new(CrosstermBackend::new(stdout()))?;
        let (event_handle, mut event_rx, sd_tx) = spawn_events();

        // Initially enter the file explorer, to let user pick file.
        // Also can't render the UI without a file to download.
        self.enter_file_explorer(&mut terminal)?;

        loop {
            
            if self.quit {
                break;
            }

            terminal.draw(|f| self.view(f))?;
            
            tokio::select! {
                
                // Some event from our bittorrent client
                Some(client_event) = self.user_rx.recv() => {
                    match client_event {
                        
                        UserCommand::TorrentResult { id, result } => {
                            let _idx = self.torrent_lookup
                                .get(&id)
                                .ok_or(color_eyre::eyre::anyhow!("torrent not found (result)"))?;
                            match result {
                                Ok(_) => {},
                                Err(e) => {},
                            }
                        },
                        
                        UserCommand::TorrentStats { id, stats } => {
                            let idx = self.torrent_lookup
                                .get(&id)
                                .ok_or(color_eyre::eyre::anyhow!("torrent not found (stats torrent)"))?;
                            self.torrents[*idx].update_torrent_stats(stats);
                        },
                        
                        UserCommand::TrackerStats { id, stats } => {
                            let idx = self.torrent_lookup
                                .get(&id)
                                .ok_or(color_eyre::eyre::anyhow!("torrent not found (stats tracker)"))?;
                            self.torrents[*idx].update_tracker_stats(stats);
                        },
                    }
                },
                
                Some(event) = event_rx.recv() => self.handle_event(event, &mut terminal)?,
            }

            debug_assert!(self.selected_idx < self.torrents.len());
        }

        sd_tx.send(()).ok();
        event_handle.await?;
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
        self.ui.torrent_table.render(f, top_row[0], &self.torrents);
        ui::render_torrent_panel(f, top_row[1], &self.torrents[self.selected_idx]);

        // // Bottom row
        // let bl = widgets::Paragraph::new("boo")
        //     .block(widgets::Block::default().title("Bottom").borders(widgets::Borders::ALL));
        // f.render_widget(bl, bottom_row[0]);
        ui::render_tracker_table(f, bottom_row[0], &self.torrents[self.selected_idx]);

        ui::render_peer_table(f, bottom_row[1], &self.torrents[self.selected_idx]);
    }

    // TODO: handle pasting a magnet link
    fn handle_event(&mut self, event: Event, terminal: &mut Terminal) -> Result<()> {
        match event {
            Event::Key(key) => {
                if key.kind != event::KeyEventKind::Press {
                    return Ok(());    
                }

                match key.code {
                    event::KeyCode::Char('q') => {
                        self.quit = true;
                    },
                    event::KeyCode::Char('n') => {
                        self.enter_file_explorer(terminal)?;
                    },
                    event::KeyCode::Up => self.prev(),
                    event::KeyCode::Down => self.next(),

                    _ => {}
                }
                

            },
            _ => {}
        }
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.ui.torrent_table.table_state.selected() {
            Some(i) => {
                if i >= self.torrents.len() - 1 {
                    0
                } else {
                    i + 1
                }
            },
            None => 0,
        };
        self.select(i);
    }

    fn prev(&mut self) {
        let i = match self.ui.torrent_table.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.torrents.len() - 1
                } else {
                    i - 1
                }
            },
            None => 0,
        };
        self.select(i);
    }

    fn select(&mut self, idx: usize) {
        self.selected_idx = idx;
        self.ui.torrent_table.table_state.select(Some(idx));
        self.ui.torrent_table.scroll_state = self.ui.torrent_table.scroll_state.position(idx);
    }

    fn enter_file_explorer(&mut self, terminal: &mut Terminal) -> Result<()> {

        loop {
            terminal.draw(|f| {
                f.render_widget_ref(self.ui.file_explorer.widget(), f.size());
            })?;

            if let Some(event) = event::read().ok() {
                match event {
                    Event::Key(key) => {
                        if key.kind != event::KeyEventKind::Press {
                            continue;
                        }
                        match key.code {
                            
                            event::KeyCode::Char('q') => {
                                self.quit = true;
                                return Ok(());
                            },
                            
                            event::KeyCode::Esc => {
                                if self.torrents.len() > 0 {
                                    return Ok(());
                                }
                            },
                            
                            event::KeyCode::Enter => {
                                let file = self.ui.file_explorer.current();
                                if file.is_dir() {
                                    self.ui.file_explorer.handle(&event)?;
                                } else {
                                    let metainfo = MetaInfo::new(file.path())?;
                                    // Sends the metainfo to the bittorrent client.
                                    self.handle.new_torrent(metainfo.clone())?;
                                    // Add the torrent to internal list.
                                    self.torrent_lookup.insert(metainfo.info_hash(), self.torrents.len());
                                    self.torrents.push(TorrentData::new(metainfo));
                                    // Select this torrent (the latest).
                                    self.select(self.torrents.len() - 1);
                                    return Ok(());
                                }
                            }

                            _ => { self.ui.file_explorer.handle(&event)?; }
                        
                        }
                    },

                    _ => continue,
                }
            }
        }
    }

    fn popup(&mut self, terminal: &mut Terminal, msg: &str, title: &str) {
        loop {

            terminal.draw(|f| {
                let block = widgets::Block::default()
                    .title(title)
                    .title_bottom(" 'Enter' to continue ")
                    .borders(widgets::Borders::ALL);
                let popup = widgets::Paragraph::new(msg)
                    .centered()
                    .block(block);
                let area = centered_rect(60, 20, f.size());
                f.render_widget(Clear, area);
                f.render_widget(popup, area);
            }).unwrap();

            if let Some(event) = event::read().ok() {
                match event {
                    Event::Key(key) => {
                        if key.kind != event::KeyEventKind::Press {
                            continue;
                        }
                        match key.code {
                            event::KeyCode::Enter => {
                                return;
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}