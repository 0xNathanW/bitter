use crate::{p2p::state::SessionState, torrent::TorrentState};

#[derive(Debug, Default)]
pub struct TorrentStats {

    pub start_time: Option<std::time::Instant>,

    pub time_elapsed: std::time::Duration,

    pub state: TorrentState,

    pub piece_stats: PieceStats,

    pub peer_stats: Vec<PeerStats>,

    pub throughput: ThroughputStats,

}

#[derive(Debug, Default)]
pub struct PieceStats {

    pub num_pieces: usize,

    pub num_pending: usize,

    pub num_downloaded: usize,

}

impl PieceStats {
    pub fn is_seed(&self) -> bool {
        self.num_downloaded == self.num_pieces
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PeerStats {

    pub address: std::net::SocketAddr,

    pub state: SessionState,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ThroughputStats {

    pub up: Counter,

    pub down: Counter,

}

impl ThroughputStats {
    pub fn reset(&mut self) {
        self.up.reset();
        self.down.reset();
    }
}

impl std::ops::AddAssign<&ThroughputStats> for ThroughputStats {
    fn add_assign(&mut self, other: &ThroughputStats) {
        self.up += other.up.round();
        self.down += other.down.round();
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Counter {
    total: u64,
    round: u64,
    avg: f64,
    peak: f64,
}

impl Counter {

    pub fn add(&mut self, n: u64) {
        self.total += n;
        self.round += n;
    }

    pub fn reset(&mut self) {
        self.avg = (self.avg * (5 - 1) as f64 / 5.0) + (self.round as f64 / 5.0);
        self.round = 0;
        if self.avg > self.peak {
            self.peak = self.avg;
        }
    }

    pub fn avg(&self) -> u64 {
        self.avg as u64
    }

    pub fn peak(&self) -> u64 {
        self.peak as u64
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn round(&self) -> u64 {
        self.round
    }

}

impl std::ops::AddAssign<u64> for Counter {
    fn add_assign(&mut self, n: u64) {
        self.add(n);
    }
}


