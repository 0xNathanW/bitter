use std::{fmt::Debug, sync::Arc, net::SocketAddr, collections::HashSet};
use tokio::{sync::mpsc::{UnboundedReceiver, UnboundedSender}, net::TcpStream};
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt, stream::SplitSink};
use crate::{
    ctx::TorrentContext, 
    p2p::{handshake::{HandshakeCodec, PROTOCOL}, PeerError, 
    message::MessageCodec, state::ConnState}, torrent::TorrentCommand, Bitfield, block::BlockInfo, 
};
use super::{Result, handshake::Handshake, state::SessionState, message::Message};

const TARGET_REQUEST_Q_LEN: usize = 4;

#[derive(Debug)]
pub enum PeerState {
    Disconnected,
    Connecting,
    Handshake,
    Connected,
}

// Commands that can be sent to a peer.
pub enum PeerCommand {

    // Safely shutdown the peer session.
    Shutdown,
    
    // Piece is available for download.
    PieceCompleted {
        idx: usize,
    }

}

#[derive(Debug)]
pub struct PeerSession {

    // Address of the peer.
    address: SocketAddr,

    // Context is a global state accessible by peers.
    torrent_ctx: Arc<TorrentContext>,
    
    // Commands to the peer.
    cmd_rx: UnboundedReceiver<PeerCommand>,
    
    // Internal send channel for disk reads.
    cmd_tx: UnboundedSender<PeerCommand>,

    // The peer's ID will be set after the handshake.
    id: Option<[u8; 20]>,

    // Bitfield of pieces the peer has.
    bitfield: Bitfield,

    // Number of pieces the peer has.
    piece_count: usize,

    // Current state of the peer.
    state: SessionState,

    // Pending block requests from peer to the client.
    block_requests_in: HashSet<BlockInfo>,

    // Pending block requests from client to peers.
    block_requests_out: HashSet<BlockInfo>,
}

impl PeerSession {

    pub fn new(address: SocketAddr, torrent_ctx: Arc<TorrentContext>) -> (PeerSession, UnboundedSender<PeerCommand>) {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let num_pieces = torrent_ctx.num_pieces;
        (
            PeerSession {
                address,
                torrent_ctx,
                cmd_rx,
                cmd_tx: cmd_tx.clone(),
                id: None,
                bitfield: Bitfield::with_capacity(num_pieces),
                piece_count: 0,
                state: SessionState::default(),
                block_requests_in: HashSet::new(),
                block_requests_out: HashSet::new(),
            }, 
            cmd_tx
        )
    }

    #[tracing::instrument(name = "peer", skip(self), fields(address = %self.address, outbound = "true"))]
    pub async fn start_session_outbound(&mut self) -> Result<()> {
        
        self.state.conn_state = ConnState::Connecting;
        let socket = TcpStream::connect(self.address).await?;
        tracing::info!("connection successful");
        
        let socket = Framed::new(socket, HandshakeCodec);
        self.establish_peer(socket, false).await
    }

    #[tracing::instrument(name = "peer", skip(self, socket), fields(address = %self.address, outbound = "false"))]
    pub async fn start_session_inbound(&mut self, socket: TcpStream) -> Result<()> {
        self.state.conn_state = ConnState::Connecting;
        let socket = Framed::new(socket, HandshakeCodec);
        self.establish_peer(socket, true).await
    }

    async fn establish_peer(&mut self, mut socket: Framed<TcpStream, HandshakeCodec>, inbound: bool) -> Result<()> {
        self.state.conn_state = ConnState::Handshaking;

        // Send handshake first if connection is outbound.
        if !inbound {
            tracing::trace!("sending handshake");
            let handshake = Handshake::new(self.torrent_ctx.info_hash.clone());
            tracing::debug!("sent handshake: {:#?}", handshake);
            socket.send(handshake).await?;
        }

        tracing::trace!("waiting for handshake");
        if let Some(handshake) = socket.next().await {
            
            let handshake = handshake?;
            tracing::trace!("recieved handshake");
            tracing::debug!("handshake: {:#?}", handshake);
            
            // Validate handshake.
            if handshake.protocol != PROTOCOL {
                tracing::warn!("incorrect protocol");
                return Err(PeerError::IncorrectProtocol);
            }
            if handshake.info_hash != self.torrent_ctx.info_hash {
                tracing::info!("incorrect info hash");
                return Err(PeerError::IncorrectInfoHash);
            }
            self.id = Some(handshake.peer_id);

            // Respond with handshake if connection is inbound.
            if inbound {
                tracing::trace!("sending handshake");
                let handshake = Handshake::new(self.torrent_ctx.info_hash.clone());
                tracing::debug!("sent handshake: {:#?}", handshake);
                socket.send(handshake).await?;
            }

            // Switch from handshake to message codec.
            socket.map_codec(|_| MessageCodec);

            // Notify context that peer is connected.
            self.torrent_ctx.cmd_tx.send(TorrentCommand::PeerConnected {
                address: self.address,
                id: handshake.peer_id,
            })?;
            tracing::info!("handshake successful");

            self.state.conn_state = ConnState::Introducing;
            // TODO: run
        } else {
            self.state.conn_state = ConnState::Disconnected;
            tracing::warn!("no handshake recieved")
        }
        
        tracing::info!("disconnecting");
        self.state.conn_state = ConnState::Disconnected;
        Ok(())
    }

    async fn run(&mut self, socket: Framed<TcpStream, MessageCodec>) -> Result<()> {
        
        let (mut sink, mut stream) = socket.split();

        {
            let guard = self.torrent_ctx.piece_selector.read().await;
            let self_pieces = guard.self_pieces();
            if self_pieces.any() {
                tracing::info!("sending bitfield");
                sink.send(Message::Bitfield(self_pieces.clone())).await?;
            }
        }

        loop { tokio::select! {

            Some(msg) = stream.next() =>{
                let msg = msg?;

                if self.state.conn_state == ConnState::Introducing {
                    if let Message::Bitfield(bitfield) = msg {
                        self.handle_bitfield(&mut sink, bitfield).await?;
                    } else {
                        self.handle_msg(&mut sink, msg).await?;
                    }
                }
            }

        }}

        #[allow(unreachable_code)]
        Ok(())
    }

    async fn handle_bitfield(
        &mut self, 
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>, 
        mut bitfield: Bitfield,
    ) -> Result<()> {
        tracing::debug!("bitfield: {:?}", bitfield);
        // Get rid of trailing values.
        bitfield.resize(self.torrent_ctx.num_pieces, false);
        tracing::info!("recieved bitfield with {} pieces", bitfield.count_ones());
        let interested = self.torrent_ctx.piece_selector.write().await.bitfield_update(&bitfield);
        self.bitfield = bitfield;
        self.piece_count = self.piece_count.count_ones() as usize;
        self.update_interest(sink, interested).await
    }

    async fn handle_msg(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
        msg: Message,
    ) -> Result<()> {

        match msg {
            Message::Bitfield(_) => {
                tracing::warn!("bitfield sent before handshake");
                return Err(PeerError::UnexpectedBitfield);
            },
            Message::KeepAlive => tracing::info!("keep alive"),
            Message::Choke => {
                if !self.state.peer_choking {
                    tracing::info!("peer now choking us");
                    // free pending blocks.
                    self.state.peer_choking = true;
                }
            },
            Message::Unchoke => {
                if self.state.peer_choking {
                    tracing::info!("peer no longer choking us");
                    self.state.peer_choking = false;

                    if self.state.interested {
                        self.prepare_download();
                        // make requests
                    }
                }
            },
            Message::Interested => {
                if !self.state.peer_interested {
                    tracing::info!("peer became interested");
                    self.state.peer_interested = true;
                    self.state.choked = false;
                    tracing::info!("unchoking peer");
                    sink.send(Message::Unchoke).await?;
                }
            },
            Message::NotInterested => {
                if self.state.peer_interested {
                    tracing::info!("peer no longer interested");
                    self.state.peer_interested = false;
                }
            },
            Message::Piece { idx, begin, block } => {},
            Message::Request { idx, begin, length } => {},
            Message::Have { idx } => {},
            Message::Port { port } => {},
            Message::Cancel { idx, begin, length } => {},
        }

        Ok(())
    }

    // Called after unchoked and are interested.
    fn prepare_download(&mut self) {

    }

    async fn make_requests(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
    ) -> Result<()> {
        
        Ok(())        
    }

    async fn update_interest(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
        interested: bool,
    ) -> Result<()> {
        
        if !self.state.interested && interested {
            self.state.interested = true;
            tracing::info!("interested in peer");
            sink.send(Message::Interested).await?;
        } else if self.state.interested && !interested {
            self.state.interested = false;
            tracing::info!("disinterested in peer");
        }
        
        Ok(())
    }
}
