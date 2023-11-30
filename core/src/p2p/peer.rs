use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle, net::TcpStream};
use super::{Result, session::{PeerSession, PeerCommand}};

#[derive(Debug)]
pub struct Peer {

    // Unique 20-byte id for peer.
    pub id: Option<[u8; 20]>,

    // Sends commands to the torrent.
    pub cmd_out: Option<UnboundedSender<PeerCommand>>,

    // Handle to the peer session.
    pub session_handle: Option<JoinHandle<Result<()>>>
}

impl Peer {

    fn new(cmd_out: UnboundedSender<PeerCommand>, handle: JoinHandle<Result<()>>) -> Peer {
        Peer {
            id: None,
            cmd_out: Some(cmd_out),
            session_handle: Some(handle),
        }
    }

    pub fn new_outbound(mut session: PeerSession, cmd_out: UnboundedSender<PeerCommand>) -> Peer {
        let handle = tokio::spawn(async move {
            session.start_session_outbound().await
        });
        Peer::new(cmd_out, handle)
    }

    pub fn new_inbound(mut session: PeerSession, cmd_out: UnboundedSender<PeerCommand>, socket: TcpStream) -> Peer {
        let handle = tokio::spawn(async move {
            session.start_session_inbound(socket).await
        });
        Peer::new(cmd_out, handle)
    }
}