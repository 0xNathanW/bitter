use std::{collections::{HashMap, HashSet}, net::SocketAddr, sync::Arc, time::Instant};
use tokio::{sync::mpsc, net::TcpStream, time};
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt, stream::SplitSink};
use crate::{
    block::{Block, BlockRequest},
    disk::DiskCommand,
    torrent::{TorrentCommand, TorrentContext},
    Bitfield,
};
use super::{*, message::*, handshake::*, state::*};

type MessageSink = SplitSink<Framed<TcpStream, MessageCodec>, Message>;

#[derive(Debug)]
pub struct PeerSession {

    address: SocketAddr,

    torrent_ctx: Arc<TorrentContext>,
    
    peer_rx: PeerRx,
    
    // Internal send channel for disk reads.
    peer_tx: PeerTx,

    // Pending block requests from peer to the client.
    requests_in: HashSet<BlockRequest>,

    // Pending block requests from client to peer.
    requests_out: HashSet<BlockRequest>,
    
    write_requests: HashMap<usize, usize>,

    bitfield: Bitfield,

    state: SessionState,

}

impl PeerSession {

    pub fn new(address: SocketAddr, torrent_ctx: Arc<TorrentContext>) -> (PeerSession, PeerTx) {

        let (peer_tx, peer_rx) = mpsc::unbounded_channel();
        let bitfield = Bitfield::repeat(false, torrent_ctx.info.num_pieces as usize);
        
        (
            PeerSession {
                address,
                torrent_ctx,
                peer_rx,
                peer_tx: peer_tx.clone(),
                bitfield,
                state: SessionState::default(),
                requests_in: HashSet::new(),
                requests_out: HashSet::new(),
                write_requests: HashMap::new(),
            }, 
            peer_tx,
        )
    }

    pub async fn start_session(&mut self, inbound_stream: Option<TcpStream>) -> Result<()> {
        let inbound = inbound_stream.is_some();
        let mut socket = self.connect(inbound_stream).await?;
        self.exchange_handshake(&mut socket, inbound).await?;
        let socket = socket.map_codec(|_| MessageCodec);
        self.run(socket).await?;
        Ok(())
    }
    
    async fn connect(&mut self, inbound_stream: Option<TcpStream>) -> Result<Framed<TcpStream, HandshakeCodec>> {
        self.state.update(|state| state.conn_state = ConnState::Connecting);
        if let Some(stream) = inbound_stream {
            Ok(Framed::new(stream, HandshakeCodec))
        } else {
            let timeout = time::Duration::from_secs(10);
            tracing::trace!("attempting outbound connection");
            let stream = time::timeout(timeout, TcpStream::connect(self.address))
                .await
                .map_err(|_| PeerError::Timeout)??;
            tracing::trace!("outbound connection successful");
            Ok(Framed::new(stream, HandshakeCodec))
        }
    }

    pub async fn disconnect(&mut self) {
        tracing::info!("disconnecting peer");
        self.state.update(|state| *state = SessionState::default());
        let _ = self.torrent_ctx.torrent_tx.send(TorrentCommand::PeerState {
            address: self.address,
            state: self.state,
        });
    }

    async fn exchange_handshake(&mut self, socket: &mut Framed<TcpStream, HandshakeCodec>, inbound: bool) -> Result<()> {
        
        let handshake = Handshake::new(self.torrent_ctx.info_hash, self.torrent_ctx.client_id);
        tracing::debug!("handshake: {:#?}", handshake);

        if !inbound {
            tracing::trace!("send handshake");
            socket.send(handshake).await?;
        }

        tracing::trace!("waiting for handshake");
        // Receive handshake.
        if let Some(Ok(handshake)) = socket.next().await {
            tracing::trace!("read: handshake");

            // Validate handshake.
            if handshake.protocol != PROTOCOL {
                return Err(PeerError::IncorrectProtocol);
            }
            if handshake.info_hash != self.torrent_ctx.info_hash {
                return Err(PeerError::IncorrectInfoHash);
            }

            // Respond with handshake if connection is inbound.
            if inbound {
                tracing::trace!("send handshake");
                socket.send(handshake).await?;
            }

            tracing::trace!("handshake successful, peer connected");
            Ok(())

        } else {
            Err(PeerError::NoHandshake)
        }
    }

    async fn run(&mut self, socket: Framed<TcpStream, MessageCodec>) -> Result<()> {

        self.state.connect_time = Some(Instant::now());
        self.state.update(|state| state.conn_state = ConnState::Introducing);

        let (mut sink, mut stream) = socket.split();
        let mut ticker = time::interval(time::Duration::from_secs(1));

        // If we have pieces, send bitfield.
        if self.torrent_ctx.picker.pieces.read().await.own_bitfield().any() {
            let bf = self.torrent_ctx.picker.pieces.read().await.own_bitfield().clone();
            self.send_message(&mut sink, Message::Bitfield(bf)).await?;
        }

        loop { tokio::select! {

            // Message from peer.
            Some(Ok(msg)) = stream.next() => self.handle_msg(&mut sink, msg).await?,

            // Command from elsewhere in application.
            Some(cmd) = self.peer_rx.recv() => {
                match cmd {

                    // From disk.
                    PeerCommand::BlockRead(block) => self.send_block(&mut sink, block).await?,

                    PeerCommand::PieceWritten(idx) => self.handle_written_piece(&mut sink, idx).await?,

                    // From torrent.
                    PeerCommand::Shutdown => {
                        tracing::trace!("session shutdown");
                        break;
                    },
                
                }
            }

            t = ticker.tick() => self.tick(t.into_std()).await?,

        }}

        Ok(())
    }

    // TODO: send multiple messages in one go, rather than flushing after each one?, particularly for requests.
    // Logs a message and sends to peer.
    #[inline(always)]
    async fn send_message(&mut self, sink: &mut MessageSink, msg: Message) -> Result<()> {
        tracing::trace!("send: {}", msg);
        sink.send(msg).await
    }

    async fn handle_msg(&mut self, sink: &mut MessageSink, msg: Message) -> Result<()> {
        tracing::trace!("read: {}", msg);

        match msg {

            // Bitfield can only be sent directly after handshake.
            Message::Bitfield(bf) => {
                if self.state.conn_state == ConnState::Introducing {
                    self.handle_bitfield(sink, bf).await?;
                } else {
                    return Err(PeerError::UnexpectedBitfield);
                }
            },
            
            Message::KeepAlive => {},
            
            Message::Choke => {
                if !self.state.peer_choking {
                    self.state.peer_choking = true;
                    // Free pending requests for other peers.
                    self.free_requests_out().await;
                }
            },
            
            Message::Unchoke => {
                if self.state.peer_choking {
                    self.state.peer_choking = false;
                    // Start to make requests if interested.
                    if self.state.interested {
                        self.make_requests(sink).await?;
                    }
                }
            },
            
            Message::Interested => {
                // TODO: Only send unchoke reciprocally.
                // TODO: limit upload slots.
                if !self.state.peer_interested {
                    self.state.peer_interested = true;
                    self.send_message(sink, Message::Unchoke).await?;
                    self.state.choked = false;
                }
            },
            
            Message::NotInterested => self.state.peer_interested = false,
            
            Message::Block(block) => {
                self.handle_block(block).await;
                self.make_requests(sink).await?;
            },
            
            // TODO: do we need to stop whole task if request is invalid?
            // Will need to match error.
            Message::Request(request) => self.handle_request(request).await?,
            
            Message::Have { idx } => self.handle_have(sink, idx).await?,
            
            Message::Port { port: _ } => { todo!() },
            
            Message::Cancel(block_info) => self.handle_cancel(block_info).await?,
        
        }

        // After bitfiled 
        if self.state.conn_state == ConnState::Introducing {

            // Check if either us or peer has any pieces.
            if self.torrent_ctx.picker.pieces.read().await.own_bitfield().not_any()
            && self.bitfield.not_any()
            {
                tracing::warn!("no pieces in connection");
                let _ = self.peer_tx.send(PeerCommand::Shutdown);
                return Ok(())
            }

            self.state.update(|state| state.conn_state = ConnState::Connected);
        }

        Ok(())
    }

    async fn handle_bitfield(&mut self, sink: &mut MessageSink, mut bitfield: Bitfield) -> Result<()> {
        tracing::trace!("peer has {}/{} pieces", bitfield.count_ones(), self.torrent_ctx.info.num_pieces);
        // Remove trailing bits.
        bitfield.resize(self.torrent_ctx.info.num_pieces as usize, false);
        // Interested if peer has pieces we don't.
        let interested = self.torrent_ctx.picker.pieces.write().await.bitfield_update(&bitfield);
        self.state.update(|state| state.num_pieces = bitfield.count_ones() as usize);
        self.bitfield = bitfield;
        self.update_interest(sink, interested).await
    }

    async fn handle_have(&mut self, sink: &mut MessageSink, idx: u32) -> Result<()> {
        // If idx is not valid, disconnect.
        if idx >= self.torrent_ctx.info.num_pieces {
            tracing::error!("have msg with invalid idx: {}", idx);
            return Err(PeerError::InvalidMessage);
        }
        // Peer already has piece.
        if self.bitfield[idx as usize] {
            return Ok(());
        }
        self.bitfield.set(idx as usize, true);
        self.state.update(|state| state.num_pieces += 1);

        let interested = self
            .torrent_ctx
            .picker
            .pieces
            .write()
            .await
            .increment_piece(idx as usize);

        self.update_interest(sink, interested).await
    }

    async fn handle_block(&mut self, block: Block) {
        
        let request = BlockRequest::from_block(&block);
        if !self.requests_out.remove(&request) {
            // TODO: penalise peer.
            // TODO: add defence against random block spamming.
            tracing::warn!("unexpected block: {:?}", &request);
            return;
        }
        
        let is_duplicate = if let Some(partial_piece) = self
            .torrent_ctx
            .picker
            .partial_pieces
            .read()
            .await
            .get(&request.piece_idx)
        {
            partial_piece.write().await.received_block(&request)  
        } else {
            // This should'nt be possible.
            // Maybe it would in end game mode, if piece completed and already written.
            // Block is being checked for in requests_out, so it should be in partial_pieces.
            tracing::warn!("received block for non-existent piece: {:?}", &request);
            return
        };

        if !is_duplicate {

            // Send a write command to disk.
            let _ = self.torrent_ctx.disk_tx
                .send(DiskCommand::WriteBlock { 
                    id: self.torrent_ctx.info_hash,
                    block,
                });
            self.write_requests.insert(request.piece_idx, request.len);
                
        } else {
            // Again, do we need to check for spamming?
            // Should allow when in end game mode.
            tracing::warn!("duplicate block: {:?}", &request);
        }
    }
    
    async fn handle_request(&mut self, request: BlockRequest) -> Result<()> {
        
        if self.state.choked {
            // TODO: maybe send peer a choke message rather than disconnect.
            tracing::error!("sending requests whilst choked");
            return Err(PeerError::InvalidMessage);
        }
        if !request.is_valid(&self.torrent_ctx.info) {
            tracing::error!("invalid request: {:?}", request);
            return Err(PeerError::InvalidMessage);
        }
        if self.requests_out.contains(&request) {
            tracing::warn!("duplicate request: {:?}", request);
            return Ok(());
        }

        self.requests_out.insert(request);
        let _ = self.torrent_ctx.disk_tx.send(DiskCommand::ReadBlock {
            id: self.torrent_ctx.info_hash,
            block: request,
            tx: self.peer_tx.clone(),
        });

        Ok(())
    }

    async fn handle_cancel(&mut self, block_info: BlockRequest) -> Result<()> {
        if !block_info.is_valid(&self.torrent_ctx.info) {
            tracing::warn!("invalid cancel: {:?}", block_info);
            return Err(PeerError::InvalidMessage);
        }
        self.requests_out.remove(&block_info);
        Ok(())
    }

    // When a piece is written to disk:
    async fn handle_written_piece(&mut self, sink: &mut MessageSink, idx: usize) -> Result<()> {
        
        if let Some(len) = self.write_requests.remove(&idx) {
            self.state.update(|state| state.throughput.down += len as u64);
        }

        if !self.bitfield[idx] {
            // Send a have message if the peer doesn't have it.
            sink.send(Message::Have { idx: idx as u32 }).await?;
        } else {
            // Cancel any requests for the piece.
            for block in self.requests_out.iter() {
                if block.piece_idx == idx {
                    sink.send(Message::Cancel(*block)).await?;
                }
            }
        }

        Ok(())
    }

    // Queue requests up to a certain target queue length.
    async fn make_requests(&mut self, sink: &mut MessageSink) -> Result<()> {

        if self.state.peer_choking || !self.state.interested {
            tracing::warn!("attempted to make requests whilst not interested or choked by peer");
            return Ok(())
        }
        
        let requests = self
            .torrent_ctx.picker
            .pick_blocks(&self.requests_out, 20, &self.bitfield)
            .await;

        for block in requests {
            tracing::trace!("send request: {:?}", block);
            self.requests_out.insert(block);
            sink.send(Message::Request(block)).await?;
        }

        Ok(())
    }

    // Remove the request and send peer block.
    async fn send_block(&mut self, sink: &mut MessageSink, block: Block) -> Result<()> {
        let request: BlockRequest = BlockRequest::from_block(&block);
        if !self.requests_in.remove(&request) {
            // TODO: think about under what circumstances this would occur.
            tracing::warn!("block read but no request: {:?}", request);
            return Ok(());
        }
        sink.send(Message::Block(block)).await?;
        self.state.update(|state| state.throughput.up += request.len as u64);
        Ok(())
    }

    // Free all requested blocks, making them available for other peers.
    async fn free_requests_out(&mut self) {
        tracing::trace!("freeing requested blocks");
        let partial_pieces = self.torrent_ctx.picker.partial_pieces.read().await;
        for request in self.requests_out.drain() {
            if let Some(partial_piece) = partial_pieces.get(&request.piece_idx) {
                partial_piece.write().await.free_block(&request);
                tracing::trace!("freed block request: {:?}", request);
            }
        }
    }
    
    // If we have BECOME interested, send a message to indicate this.
    async fn update_interest(&mut self, sink: &mut MessageSink, interested: bool) -> Result<()> {
        if !self.state.interested && interested {
            self.state.interested = true;
            self.send_message(sink, Message::Interested).await?;
        } else if self.state.interested && !interested {
            self.state.interested = false;
        }
        Ok(())
    }

    async fn tick(&mut self, time: Instant) -> Result<()> {
    
        // Disconnect for inactivity.
        if !self.state.interested 
        && !self.state.peer_interested 
        && time.saturating_duration_since(self.state.connect_time.unwrap())
            >= time::Duration::from_secs(30)
        {
            return Err(PeerError::Timeout)
        }

        // Send stats if there is a state change.
        if self.state.changed {
            let _ = self.torrent_ctx.torrent_tx.send(TorrentCommand::PeerState {
                address: self.address,
                state: self.state,
            });
        }
        self.state.throughput.reset();

        Ok(())
    }
}
