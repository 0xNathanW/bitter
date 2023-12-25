use tokio::sync::mpsc::UnboundedSender;
use crate::{torrent::CommandToTorrent, picker::Picker};

#[derive(Debug)]
pub struct TorrentContext {
    
    // The info hash for this torrent.
    pub info_hash:      [u8; 20],

    // The client ID for this client.
    pub client_id:      [u8; 20],

    pub picker: Picker,

    // The number of pieces in this torrent.
    pub num_pieces:     usize,

    // The total size of this torrent in bytes.
    pub total_size:     u64,

    // Commands to the peer.
    pub cmd_tx:        UnboundedSender<CommandToTorrent>,
    
}
