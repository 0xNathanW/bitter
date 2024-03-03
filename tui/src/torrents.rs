use bittorrent::{MetaInfo, TorrentID, TorrentStats};
use ratatui::{layout::{Constraint, Rect}, style::{Color, Style}, text::Text, widgets};

// Information the user may want to know about a torrent.
struct TorrentInfo {
    id:   TorrentID,
    name: String,
    size: String,
    stats: TorrentStats,
}

impl TorrentInfo {
    pub fn new(metainfo: MetaInfo) -> Self {
        Self {
            id: metainfo.info_hash(),
            name: metainfo.name().to_string(),
            size: metainfo.size_fmt(),
            stats: TorrentStats::default(),
        }
    }

    fn row_data(&self) -> [String; 4] {
        [
            self.name.clone(),
            self.size.clone(),
            self.stats.throughput.down.avg().to_string(),
            self.stats.throughput.up.avg().to_string(),
        ]
    }
}

pub struct TorrentTable {
    torrents: Vec<TorrentInfo>,
    table_state: widgets::TableState,
    scroll_state: widgets::ScrollbarState,
}

impl TorrentTable {
    pub fn new() -> Self {
        Self {
            torrents: Vec::new(),
            table_state: widgets::TableState::default().with_selected(0),
            scroll_state: widgets::ScrollbarState::default(),   
        }
    }

    pub fn add_torrent(&mut self, metainfo: MetaInfo) {
        self.torrents.push(TorrentInfo::new(metainfo));
    }

    pub fn remove_torrent(&mut self, id: TorrentID) {
        self.torrents.retain(|t| t.id != id);
    }

    pub fn update_torrent(&mut self, id: TorrentID, stats: TorrentStats) {
        if let Some(i) = self.torrents.iter().position(|t| t.id == id) {
            self.torrents[i].stats = stats;
        }
    }

    pub fn next(&mut self) {
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
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i);
    }

    pub fn prev(&mut self) {
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
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i);
    }

    pub fn render(&mut self, f: &mut ratatui::Frame, area: Rect) {
        
        let header = ["Name", "Size", "Down", "Up"]
            .iter()
            .cloned()
            .map(widgets::Cell::from)
            .collect::<widgets::Row>()
            .style(Style::new().fg(Color::Black).bg(Color::Blue))
            .height(1);

        let rows = self.torrents
            .iter()
            .map(|torrent| {
                torrent
                    .row_data()
                    .iter()
                    .cloned()
                    .map(|x| widgets::Cell::from(Text::from(x)))
                    .collect::<widgets::Row>()
                    .height(1)
            });

        let table = widgets::Table::new(rows, Constraint::from_percentages([25, 25, 25, 25]))
            .header(header)
            .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED))
            .highlight_spacing(widgets::HighlightSpacing::Always);

        f.render_stateful_widget(table, area, &mut self.table_state);    
    }
}
