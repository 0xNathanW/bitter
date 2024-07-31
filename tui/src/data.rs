use std::time::{Duration, Instant};
use bittorrent::{stats::{PeerStats, PieceStats, TorrentStats}, ConnState, MetaInfo, TorrentState, ID};

// Information the user may want to know about a torrent.
#[derive(Debug)]
pub struct TorrentData {

    pub id: ID,
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
            id: metainfo.info_hash(),
            name: metainfo.name().to_string(),
            size: metainfo.size_fmt(),
            num_pieces: metainfo.num_pieces() as usize,
            history_up: vec![0; 200],
            history_down: vec![0; 200],
            data: TorrentStats {
                start_time: Instant::now(),
                time_elapsed: Duration::default(),
                state: TorrentState::default(),
                piece_stats: PieceStats {
                    num_pieces: metainfo.num_pieces() as usize,
                    num_pending: 0,
                    num_downloaded: 0,
                },
                peer_stats: Vec::new(),
                throughput: Default::default(),
            }
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
            .filter(|peer| peer.state.conn_state != ConnState::Disconnected)
            .map(|peer| {
                
                [
                    peer.address.to_string(),
                    peer_flags(&peer),
                    format!("{:.0}%", peer.state.num_pieces as f64 / self.num_pieces as f64 * 100.0),
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

    fn time_elapsed(&self) -> String {
        let total_secs = self.data.time_elapsed.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

fn peer_flags(peer: &PeerStats) -> String {
    
    let mut flags = String::new();
    let s = &peer.state;

    // ?: your client unchoked the peer but the peer is not interested
    if !s.choked && !s.peer_interested {
        flags.push('?');
    }
    if s.interested {
        if s.peer_choking {
            // d: your client wants to download, but peer doesn't want to send (interested and choked)
            flags.push('d');
        } else {
            // D: currently downloading from the peer (interested and not choked)
            flags.push('D');
        }
    }
    // K: peer unchoked your client, but your client is not interested
    if !s.peer_choking && !s.interested {
        flags.push('K');
    }
    // U: currently uploading to the peer (interested and not choked)
    if s.peer_interested {
        if s.choked {
            // u: the peer wants your client to upload, but your client doesn't want to (interested and choked)
            flags.push('u');
        } else {
            // U: currently uploading to the peer (interested and not choked)
            flags.push('U');
        }
    }

    flags
}

/*
Flags displays various letters, each carrying a special meaning about the state of the connection:

?: your client unchoked the peer but the peer is not interested

D: currently downloading from the peer (interested and not choked)

d: your client wants to download, but peer doesn't want to send (interested and choked)

E: peer is using Protocol Encryption (all traffic)

e: peer is using Protocol Encryption (handshake)

F: peer was involved in a hashfailed piece (not necessarily a bad peer, just involved) (TODO)

H: peer was obtained through DHT

h: peer connection established via UDP hole-punching

I: peer established an incoming connection (TODO)

K: peer unchoked your client, but your client is not interested

L: peer has been or discovered via Local Peer Discovery

O: optimistic unchoke

P: peer is communicating and transporting data over uTP

S: peer is snubbed

U: currently uploading to the peer (interested and not choked)

u: the peer wants your client to upload, but your client doesn't want to (interested and choked)

X: peer was included in peer lists obtained through Peer Exchange (PEX) 
*/