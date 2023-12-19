use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle, net::TcpStream};
use super::{Result, session::{PeerSession, CommandToPeer}};

#[derive(Debug)]
pub struct PeerHandle {

    // Unique 20-byte id for peer.
    pub id: Option<[u8; 20]>,

    // Sends commands to the torrent.
    pub cmd_out: Option<UnboundedSender<CommandToPeer>>,

    // Handle to the peer session.
    pub session_handle: Option<JoinHandle<Result<()>>>,
    
}

impl PeerHandle {

    fn new(cmd_out: UnboundedSender<CommandToPeer>, handle: JoinHandle<Result<()>>) -> PeerHandle {
        PeerHandle {
            id: None,
            cmd_out: Some(cmd_out),
            session_handle: Some(handle),
        }
    }

    pub fn start_session(mut session: PeerSession, cmd_out: UnboundedSender<CommandToPeer>, socket: Option<TcpStream>) -> PeerHandle {
        let handle = tokio::spawn(async move {
            session.start_session(socket).await
        });
        PeerHandle::new(cmd_out, handle)
    }
}