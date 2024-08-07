use std::{collections::HashMap, io::{stdout, Stdout}};
use bittorrent::{Handle, UserCommand, MetaInfo, ID, UserRx};
use crossterm::event::{self, Event};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, layout::Layout, widgets, Frame};
use color_eyre::Result;
use crate::{data::TorrentData, ui};

pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct App {

    // Handle to the bittorrent client.
    client:   Handle,

    // Channel to recieve messages from the bittorrent client.
    user_rx:  UserRx,

    // List of active torrents, in vec for order.
    torrents: Vec<TorrentData>,

    // Maps torrent id to index in the torrents vector.
    torrent_lookup: HashMap<ID, usize>,

    enter_file_explorer: bool,

    file_explorer: ratatui_explorer::FileExplorer,

    table_state: widgets::TableState,

    scroll_state: widgets::ScrollbarState,

    // Index of the currently selected torrent.
    selected_idx: usize,

    // Flag to quit the app on next loop.
    quit: bool,

}

impl App {

    pub fn new() -> Result<Self> {
        // TODO: config
        let (handle, user_rx) = bittorrent::start_client(None);
        let file_explorer = ratatui_explorer::FileExplorer::with_theme(
            ratatui_explorer::Theme::default()
                .add_default_title()
                .with_title_bottom(|_| " 'q': quit | 'enter': select | 'esc': back ".into())
        )?;
        
        Ok(Self {
            client: handle,
            user_rx,
            torrents: Vec::new(),
            torrent_lookup: HashMap::new(),
            enter_file_explorer: false,
            file_explorer,
            table_state: widgets::TableState::default().with_selected(0),
            scroll_state: widgets::ScrollbarState::default(),
            selected_idx: 0,
            quit: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {

        let mut terminal = ratatui::Terminal::new(CrosstermBackend::new(stdout()))?;
        let mut events = event::EventStream::new();

        // Initially enter the file explorer, to let user pick file.
        // Also can't render the UI without a file to download.
        self.enter_file_explorer(&mut terminal)?;

        loop {
            
            tokio::select! { biased;
                
                Some(event) = events.next() => {
                    let event = event?;
                    self.handle_event(event).await?;
                },

                // Some event from our bittorrent client
                Some(client_event) = self.user_rx.recv() => {
                    match client_event {
                        
                        UserCommand::TorrentFinished { id } => {
                            if let Some(idx) = self.torrent_lookup.get(&id) {
                                self.torrents.remove(*idx);
                                // Expensive but rare.
                                self.torrent_lookup.clear();
                                for (i, torrent) in self.torrents.iter().enumerate() {
                                    self.torrent_lookup.insert(torrent.id, i);
                                }
                            }
                        },
                        
                        UserCommand::TorrentStats { id, stats } => {
                            if let Some(idx) = self.torrent_lookup.get(&id) {
                                self.torrents[*idx].update_torrent_stats(stats);
                            }
                        },
                    }
                },
            }
            
            if self.enter_file_explorer {
                self.enter_file_explorer(&mut terminal)?;
                self.enter_file_explorer = false;
            }

            if self.quit {
                break;
            }

            terminal.draw(|f| self.view(f))?;

            debug_assert!(self.selected_idx < self.torrents.len());
        }

        Ok(())
    }

    pub async fn shutdown(self) -> Result<()> {
        self.client.shutdown().await?;
        Ok(())
    }

    // Main rendering function.
    fn view(&mut self, f: &mut Frame) {

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

        ui::render_torrent_table(f, rows[0], &mut self.table_state, &self.torrents);
        ui::render_torrent_panel(f, bottom_row[0], &self.torrents[self.selected_idx]);
        ui::render_peer_table(f, bottom_row[1], &self.torrents[self.selected_idx]);
    }

    // TODO: handle pasting a magnet link
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                if key.kind != event::KeyEventKind::Press {
                    return Ok(());
                }

                match key.code {
                    event::KeyCode::Char('q') => self.quit = true,
                    event::KeyCode::Char('n') => self.enter_file_explorer = true,
                    event::KeyCode::Char('r') => {
                        self.client.remove_torrent(self.torrents[self.selected_idx].id).await?;
                        self.remove_torrent(self.selected_idx);
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
        let i = match self.table_state.selected() {
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
        let i = match self.table_state.selected() {
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
        self.table_state.select(Some(idx));
        self.scroll_state = self.scroll_state.position(idx);
    }

    fn remove_torrent(&mut self, idx: usize) {
        self.torrents.remove(idx);
        // Expensive but rare.
        self.torrent_lookup.clear();
        for (i, torrent) in self.torrents.iter().enumerate() {
            self.torrent_lookup.insert(torrent.id, i);
        }
        if self.torrents.len() > 0 {
            self.select(0);
        } else {
            self.enter_file_explorer = true;
        }
    }

    fn enter_file_explorer(&mut self, terminal: &mut Terminal) -> Result<()> {

        loop {
            terminal.draw(|f| {
                f.render_widget_ref(self.file_explorer.widget(), f.size());
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
                                } else {
                                    self.quit = true;
                                    return Ok(());
                                }
                            },
                            
                            event::KeyCode::Enter => {
                                let file = self.file_explorer.current();
                                if file.is_dir() {
                                    self.file_explorer.handle(&event)?;
                                } else {
                                    let metainfo = MetaInfo::new(file.path())?;
                                    let id = metainfo.info_hash();
                                    // Add the torrent to internal list.
                                    self.torrent_lookup.insert(id, self.torrents.len());
                                    self.torrents.push(TorrentData::new(metainfo.clone()));
                                    // Sends the metainfo to the bittorrent client.
                                    self.client.new_torrent(metainfo)?;
                                    // Select this torrent (the latest).
                                    self.select(self.torrents.len() - 1);
                                    return Ok(());
                                }
                            }

                            _ => { self.file_explorer.handle(&event)?; }
                        
                        }
                    },

                    _ => continue,
                }
            }
        }
    }

//     fn popup(&mut self, terminal: &mut Terminal, msg: &str, title: &str) {
//         loop {

//             terminal.draw(|f| {
//                 let block = widgets::Block::default()
//                     .title(title)
//                     .title_bottom(" 'Enter' to continue ")
//                     .borders(widgets::Borders::ALL);
//                 let popup = widgets::Paragraph::new(msg)
//                     .centered()
//                     .block(block);
//                 let area = centered_rect(60, 20, f.size());
//                 f.render_widget(widgets::Clear, area);
//                 f.render_widget(popup, area);
//             }).unwrap();

//             if let Some(event) = event::read().ok() {
//                 match event {
//                     Event::Key(key) => {
//                         if key.kind != event::KeyEventKind::Press {
//                             continue;
//                         }
//                         match key.code {
//                             event::KeyCode::Enter => {
//                                 return;
//                             },
//                             _ => {}
//                         }
//                     },
//                     _ => {}
//                 }
//             }
//         }
//     }
}

// fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
//     let popup_layout = Layout::vertical([
//         Constraint::Percentage((100 - percent_y) / 2),
//         Constraint::Percentage(percent_y),
//         Constraint::Percentage((100 - percent_y) / 2),
//     ])
//     .split(r);

//     Layout::horizontal([
//         Constraint::Percentage((100 - percent_x) / 2),
//         Constraint::Percentage(percent_x),
//         Constraint::Percentage((100 - percent_x) / 2),
//     ])
//     .split(popup_layout[1])[1]
// }