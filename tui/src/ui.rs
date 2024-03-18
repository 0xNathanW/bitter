use bittorrent::stats::TrackerStats;
use ratatui::{widgets, prelude::*};
use ratatui_explorer::{FileExplorer, Theme};
use color_eyre::Result;
use crate::data::TorrentData;

pub struct UiStates {

    pub torrent_table: TorrentTableState,

    pub file_explorer: FileExplorer,
}

impl UiStates {
    pub fn new() -> Result<Self> {
        let file_explorer = FileExplorer::with_theme(
            Theme::default()
                .add_default_title()
                .with_title_bottom(|_| " 'q': quit | 'enter': select | 'esc': back ".into())
        )?;
        
        Ok(Self {
            torrent_table: TorrentTableState::new(),
            file_explorer,
        })
    }
}

pub struct TorrentTableState {
    pub table_state: widgets::TableState,
    pub scroll_state: widgets::ScrollbarState,
}

impl TorrentTableState {
    pub fn new() -> Self {
        Self {
            table_state: widgets::TableState::default().with_selected(0),
            scroll_state: widgets::ScrollbarState::default(),   
        }
    }

    // Renders the top left panel.
    pub fn render(
        &mut self, 
        f: &mut ratatui::Frame, 
        area: Rect, 
        torrents: &Vec<TorrentData>
    ) {

        let block = widgets::Block::default()
            .title(" Torrents ")
            .borders(widgets::Borders::ALL);

        let header = ["Name", "Size", "Status", "Progress", "Time"]
            .iter()
            .cloned()
            .map(widgets::Cell::from)
            .collect::<widgets::Row>()
            .style(Style::new().underlined())
            .height(1);

        let rows = torrents
            .iter()
            .map(|torrent| {
                torrent
                    .torrent_table_row_data()
                    .iter()
                    .cloned()
                    .map(|x| widgets::Cell::from(Text::from(x)))
                    .collect::<widgets::Row>()
                    .height(1)
            });

        let table = widgets::Table::new(rows, Constraint::from_percentages([20, 20, 20, 20, 20]))
            .block(block)
            .header(header)
            .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED))
            .highlight_spacing(widgets::HighlightSpacing::Always);

        f.render_stateful_widget(table, area, &mut self.table_state);
    }
}

// Renders the top left panel.
pub fn render_torrent_panel(f: &mut ratatui::Frame, area: Rect, data: &TorrentData) {

    let rows = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                ratatui::layout::Constraint::Length(3),
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(50),
            ]
            .as_ref(),
        )
        .split(area);

    let progress_title = format!(
        " Progress: {{ pieces: {}/{} | eta: {} }} ",
        data.data.piece_stats.num_downloaded,
        data.num_pieces,
        data.eta(),
    );

    let gauge_block = widgets::Block::default()
        .title(progress_title.as_str())
        .borders(widgets::Borders::ALL);

    let gauge = widgets::Gauge::default()
        .block(gauge_block)
        .gauge_style(Style::default().fg(Color::Green))
        .percent(data.percent_complete());

    let download_title = format!(
        " Download: {{ {:.2} KB/s | peak: {:.2} KB/s }} ",
        data.data.throughput.down.avg() as f64 / 1024.0,
        data.data.throughput.down.peak() as f64 / 1024.0,
    );

    let download_block = widgets::Block::default()
        .title(download_title.as_str())
        .borders(widgets::Borders::ALL);

    let download = widgets::Sparkline::default()
        .block(download_block)
        .data(&data.history_down)
        .style(Style::default().fg(Color::Blue));

    let upload_title = format!(
        " Upload: {{ {:.2} KB/s | peak: {:.2} KB/s }} ",
        data.data.throughput.up.avg() as f64 / 1024.0,
        data.data.throughput.up.peak() as f64 / 1024.0,
    );

    let upload_block = widgets::Block::default()
        .title(upload_title.as_str())
        .borders(widgets::Borders::ALL);

    let upload = widgets::Sparkline::default()
        .block(upload_block)
        .data(&data.history_up)
        .style(Style::default().fg(Color::Red));

    f.render_widget(gauge, rows[0]);
    f.render_widget(download, rows[1]);
    f.render_widget(upload, rows[2]);
}

pub fn render_peer_table(f: &mut ratatui::Frame, area: Rect, data: &TorrentData) {

    let block = widgets::Block::default()
        .title(" Peers ")
        .borders(widgets::Borders::ALL);

    let header = ["Address", "State", "Coverage", "D KB/s", "U KB/s"]
        .iter()
        .cloned()
        .map(widgets::Cell::from)
        .collect::<widgets::Row>()
        .style(Style::new().underlined())
        .height(1);

    let peer_row_data = data.peer_table_row_data();
    let rows = peer_row_data
        .iter()
        .map(|peer| {
            peer
                .iter()
                .cloned()
                .map(|x| widgets::Cell::from(Text::from(x)))
                .collect::<widgets::Row>()
                .height(1)
        }); 

    let table = widgets::Table::new(rows, Constraint::from_percentages([20; 5]))
        .block(block)
        .header(header)
        .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED))
        .highlight_spacing(widgets::HighlightSpacing::Always);

    f.render_widget(table, area);
}

pub fn render_tracker_table(f: &mut ratatui::Frame, area: Rect, data: &TorrentData) {

    let block = widgets::Block::default()
        .title(" Trackers ")
        .borders(widgets::Borders::ALL);

    let header = ["URL", "Num Peers"]
        .iter()
        .cloned()
        .map(widgets::Cell::from)
        .collect::<widgets::Row>()
        .style(Style::new().underlined())
        .height(1);

    let tracker_row_data = data.tracker_table_row_data();
    let rows = tracker_row_data
        .iter()
        .map(|tracker| {
            tracker
                .iter()
                .cloned()
                .map(|x| widgets::Cell::from(Text::from(x)))
                .collect::<widgets::Row>()
                .height(1)
        });

    let table = widgets::Table::new(rows, Constraint::from_percentages([50, 50]))
        .block(block)
        .header(header)
        .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED))
        .highlight_spacing(widgets::HighlightSpacing::Always);

    f.render_widget(table, area);

}