use bittorrent::{stats::TorrentStats, ConnState, MetaInfo, TorrentState};

// Information the user may want to know about a torrent.
#[derive(Debug, Default)]
pub struct TorrentData {

    pub name: String,
    pub size: String,
    pub num_pieces: usize,
    
    pub data: TorrentStats,
    pub history_up: Vec<u64>,
    pub history_down: Vec<u64>,
    
}

impl TorrentData {
    pub fn new(metainfo: MetaInfo) -> Self {
        Self {
            name: metainfo.name().to_string(),
            size: metainfo.size_fmt(),
            num_pieces: metainfo.num_pieces() as usize,
            history_up: vec![0; 200],
            history_down: vec![0; 200],
            ..Default::default()
        }
    }

    pub fn update_torrent_stats(&mut self, stats: TorrentStats) {
        self.history_up.pop();
        self.history_up.insert(0, stats.throughput.up.avg());
        self.history_down.pop();
        self.history_down.insert(0, stats.throughput.down.avg());
        self.data = stats;
    }

    pub fn torrent_table_row_data(&self) -> [String; 5] {
        [
            self.name.clone(),
            self.size.clone(), 
            match self.data.state {
                TorrentState::Downloading => "downloading".to_string(),
                TorrentState::Seeding => "seeding".to_string(),
                TorrentState::Paused => "paused".to_string(),
                TorrentState::Announcing => "announcing".to_string(),
                TorrentState::Checking => "checking".to_string(),
                TorrentState::Stopped => "stopped".to_string(),
            },
            format!("{:.1}%", self.percent_complete()),
            self.time_elapsed(),
        ]
    }

    pub fn peer_table_row_data(&self) -> Vec<[String; 5]> {
        
        let mut peer_stats = self.data.peer_stats.clone();
        peer_stats.sort_by(|a, b| {
            b.state.throughput.down.avg().partial_cmp(&a.state.throughput.down.avg()).unwrap()
        });

        peer_stats
            .iter()
            .map(|peer| {
                [
                    peer.address.to_string(),
                    match peer.state.conn_state {
                        ConnState::Connected => "connected".to_string(),
                        ConnState::Handshaking => "handshaking".to_string(),
                        ConnState::Disconnected => "disconnected".to_string(),
                        ConnState::Introducing => "introducing".to_string(),
                        ConnState::Connecting => "connecting".to_string(),
                    },
                    format!("{:.1}%", peer.state.num_pieces as f64 / self.num_pieces as f64 * 100.0),
                    format!("{:.2}", peer.state.throughput.down.avg() as f64 / 1024.0),
                    format!("{:.2}", peer.state.throughput.up.avg() as f64 / 1024.0),
                ]
        }).collect()
    }

    pub fn percent_complete(&self) -> u16 {
        (
            (
                self.data.piece_stats.num_downloaded as f64
                / self.num_pieces as f64
            )
            * 100.0
        ) as u16
    }

    // TODO: obviously wrong.
    pub fn eta(&self) -> String {
        if self.data.throughput.down.avg() == 0 {
            "∞".to_string()
        } else {
            let time = self.data.piece_stats.num_pending as f64
                / self.data.throughput.down.avg() as f64;
            if time.is_infinite() {
                "∞".to_string()
            } else {
                let time = time as u64;
                let hours = time / 3600;
                let minutes = (time % 3600) / 60;
                let seconds = time % 60;
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            }
        }
    }

    pub fn time_elapsed(&self) -> String {
        let total_secs = self.data.time_elapsed.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    pub fn download_fmt(&self) -> String {
        format!(
            "{:.2} {}",
            self.data.throughput.down.avg() as f64 / 1024.0,
            "KB/s"
        )
    }

    pub fn upload_fmt(&self) -> String {
        format!(
            "{:.2} {}",
            self.data.throughput.up.avg() as f64 / 1024.0,
            "KB/s"
        )
    }
}

