use std::{fmt::Debug, sync::Arc, net::SocketAddr, collections::HashSet, alloc::handle_alloc_error};
use tokio::{sync::mpsc::{UnboundedReceiver, UnboundedSender}, net::TcpStream};
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt, stream::{SplitSink, SplitStream}};
use crate::{
    ctx::TorrentContext, 
    p2p::{handshake::{HandshakeCodec, PROTOCOL}, PeerError, 
    message::MessageCodec, state::ConnState}, torrent::CommandToTorrent, Bitfield, block::BlockInfo, 
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
pub enum CommandToPeer {

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
    cmd_rx: UnboundedReceiver<CommandToPeer>,
    
    // Internal send channel for disk reads.
    cmd_tx: UnboundedSender<CommandToPeer>,

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

    pub fn new(address: SocketAddr, torrent_ctx: Arc<TorrentContext>) -> (PeerSession, UnboundedSender<CommandToPeer>) {
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

    // TODO: add stat changes onto read/write functions.
    fn on_read_message(&mut self, msg: &Message) {
        tracing::info!("read: {}", msg);
    }

    async fn write_message(&mut self, sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>, msg: Message) -> Result<()> {
        tracing::info!("send: {}", msg);
        sink.send(msg).await?;
        Ok(())
    }
    
    #[tracing::instrument(name = "peer", skip(self, inbound_stream), fields(address = %self.address))]
    pub async fn start_session(&mut self, inbound_stream: Option<TcpStream>) -> Result<()> {
        self.state.conn_state = ConnState::Connecting;
        let inbound = inbound_stream.is_some();
        let socket = if let Some(stream) = inbound_stream {
            Framed::new(stream, HandshakeCodec)
        } else {
            let stream = TcpStream::connect(self.address).await?;
            tracing::trace!("outbound connection successful");
            Framed::new(stream, HandshakeCodec)
        };
        self.establish_peer(socket, inbound).await
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
                tracing::warn!("incorrect protocol in handshake");
                return Err(PeerError::IncorrectProtocol);
            }
            if handshake.info_hash != self.torrent_ctx.info_hash {
                tracing::info!("incorrect info hash in handshake");
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
            let msg_socket = socket.map_codec(|_| MessageCodec);

            // Notify context that peer is connected.
            self.torrent_ctx.cmd_tx.send(CommandToTorrent::PeerConnected {
                address: self.address,
                id: handshake.peer_id,
            })?;
            tracing::info!("handshake successful");

            self.state.conn_state = ConnState::Introducing;
            self.run(msg_socket).await?;
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

        loop { tokio::select! {

            Some(msg) = stream.next() =>{
                let msg = msg?;
                self.on_read_message(&msg);
                if self.state.conn_state == ConnState::Introducing {
                    if let Message::Bitfield(bitfield) = msg {
                        self.handle_bitfield(&mut sink, bitfield).await?;
                    } else {
                        self.handle_msg(&mut sink, msg).await?;
                    }
                } else {
                    self.handle_msg(&mut sink, msg).await?;
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
        bitfield.resize(self.torrent_ctx.num_pieces, false);
        let interested = self.torrent_ctx.picker.piece_picker.write().await.bitfield_update(&bitfield);
        self.bitfield = bitfield;
        self.piece_count = self.piece_count.count_ones() as usize;
        self.update_interest(sink, interested).await
    }

    // Generic message handler.
    async fn handle_msg(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
        msg: Message,
    ) -> Result<()> {

        match msg {
            Message::Bitfield(_) => {
                tracing::warn!("unexpected bitfield");
                return Err(PeerError::UnexpectedBitfield);
            },
            Message::KeepAlive => {},
            Message::Choke => {
                if !self.state.peer_choking {
                    self.state.peer_choking = true;
                    // free pending blocks.
                }
            },
            Message::Unchoke => {
                if self.state.peer_choking {
                    self.state.peer_choking = false;
                    // Start to make requests if we are interested.
                    if self.state.interested {
                        self.make_requests(sink).await?;
                    }
                }
            },
            Message::Interested => {
                if !self.state.peer_interested {
                    self.state.peer_interested = true;
                    self.write_message(sink, Message::Unchoke).await?;
                    self.state.choked = false;
                }
            },
            Message::NotInterested => {
                if self.state.peer_interested {
                    self.state.peer_interested = false;
                }
            },
            Message::Piece { idx, begin, block } => {
                self.handle_block(idx, begin, block).await?;
            },
            Message::Request(block) => {},
            Message::Have { idx } => {},
            Message::Port { port } => {},
            Message::Cancel(block) => {},
        }

        Ok(())
    }

    async fn handle_block(
        &mut self,
        piece_idx: usize,
        offset: usize,
        block: Vec<u8>,
    ) -> Result<()> {
        let block = BlockInfo {
            piece_idx,
            offset,
            len: block.len() as u32,
        };
        self.block_requests_out.remove(&block);

        Ok(())    
    }
    
    // Queue requests up to a certain target queue length.
    async fn make_requests(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
    ) -> Result<()> {
        if self.state.peer_choking {
            return Ok(())
        }
        if !self.state.interested {
            return Ok(())
        }
        let requests = self.torrent_ctx.picker.pick_blocks(&self.block_requests_out, 4).await;
        for block in requests.into_iter() {
            self.block_requests_out.insert(block);
            self.write_message(sink, Message::Request(block)).await?;
        }
        Ok(())
    }
    
    // Send message to peer if we become interested.
    async fn update_interest(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
        interested: bool,
    ) -> Result<()> {
        // Become interested.
        if !self.state.interested && interested {
            self.state.interested = true;
            self.write_message(sink, Message::Interested).await?;
        } else if self.state.interested && !interested {
            self.state.interested = false;
        }
        Ok(())
    }
}
