use crate::torrent::CommandToTorrent;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub enum CommandToDisk {
    WriteBlock { idx: usize, offset: usize, data: Vec<u8> }   
}

pub struct Disk {
    cmd_tx: UnboundedSender<CommandToTorrent>,
    cmd_rx: UnboundedReceiver<CommandToDisk>,
}

impl Disk {
    pub fn new(torrent_cmd_tx: UnboundedSender<CommandToTorrent>) -> (Self, UnboundedSender<CommandToDisk>) { 
        let (cmd_tx, cmd_rx) = unbounded_channel();
        (Self { cmd_tx: torrent_cmd_tx, cmd_rx }, cmd_tx)
    }
}

