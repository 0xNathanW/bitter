use std::{collections::HashSet, net::SocketAddr, sync::Arc};
use tokio::{sync::mpsc, net::TcpStream, time};
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt, stream::SplitSink};
use crate::{block, torrent, Bitfield, fs, picker::partial_piece::BlockState};
use super::{*, message::*, handshake::*, state::*};

const TARGET_REQUEST_Q_LEN: usize = 4;

type MessageSink = SplitSink<Framed<TcpStream, MessageCodec>, Message>;

#[derive(Debug)]
pub enum PeerState {
    Disconnected,
    Connecting,
    Handshake,
    Connected,
}

#[derive(Debug)]
pub struct PeerSession {

    // The peer's IP address.
    address: SocketAddr,
    
    // The peer's ID will be set after the handshake.
    id: Option<[u8; 20]>,

    // Context is a read only state accessible by all peers.
    torrent_ctx: Arc<torrent::TorrentContext>,
    
    // Commands to the peer.
    peer_rx: PeerRx,
    
    // Internal send channel for disk reads.
    peer_tx: PeerTx,

    // Pending block requests from peer to the client.
    requests_in: HashSet<block::BlockInfo>,

    // Pending block requests from client to peer.
    requests_out: HashSet<block::BlockInfo>,
    
    // Bitfield of pieces the peer currently has.
    bitfield: Bitfield,

    state: SessionState,

    
}

impl PeerSession {

    pub fn new(address: SocketAddr, torrent_ctx: Arc<torrent::TorrentContext>) -> (PeerSession, PeerTx) {

        let (peer_tx, peer_rx) = mpsc::unbounded_channel();
        let num_pieces = torrent_ctx.info.num_pieces as usize;
        
        (
            PeerSession {
                address,
                torrent_ctx,
                peer_rx,
                peer_tx: peer_tx.clone(),
                id: None,
                bitfield: Bitfield::repeat(false, num_pieces),
                state: SessionState::default(),
                requests_in: HashSet::new(),
                requests_out: HashSet::new(),
            }, 
            peer_tx,
        )
    }

    // TODO: send multiple messages in one go, rather than flushing after each one?, particularly for requests.
    // Logs a message and sends to peer.
    async fn send_message(&mut self, sink: &mut MessageSink, msg: Message) -> Result<()> {
        tracing::info!("send: {}", msg);
        sink.send(msg).await?;
        Ok(())
    }
    
    async fn send_handshake(
        &mut self,
        sink: &mut Framed<TcpStream, HandshakeCodec>,
    ) -> Result<()> {
        let handshake = Handshake::new(self.torrent_ctx.info_hash, self.torrent_ctx.client_id);
        tracing::info!("sending handshake");
        tracing::debug!("sent handshake: {:#?}", handshake);
        sink.send(handshake).await?;
        Ok(())
    }

    // This logging setup is a bit of a mess.
    #[tracing::instrument(name = "peer", skip(self, inbound_stream), fields(address = %self.address))]
    pub async fn start_session(&mut self, inbound_stream: Option<TcpStream>) -> Result<()> {
        let socket = match self.establish_peer(inbound_stream).await {
            socket @ Ok(_) => socket,
            Err(e) => {
                tracing::error!("failed to establish peer: {}", e);
                Err(e)
            }
        }?;
        self.run(socket).await.map_err(|e| {
            tracing::error!("peer session error: {}", e);
            e
        })?;
        self.state.conn_state = ConnState::Disconnected;
        Ok(())
    }

    async fn establish_peer(&mut self, inbound_stream: Option<TcpStream>) -> Result<Framed<TcpStream, MessageCodec>> {
        
        self.state.conn_state = ConnState::Connecting;
        let inbound = inbound_stream.is_some();
        let mut socket = if let Some(stream) = inbound_stream {
            Framed::new(stream, HandshakeCodec)
        } else {
            
            let stream = time::timeout(
                time::Duration::from_secs(15),
                TcpStream::connect(self.address),
            )
                .await
                .map_err(|_| PeerError::Timeout)??;
            
            tracing::trace!("outbound connection successful");
            Framed::new(stream, HandshakeCodec)
        };

        self.state.conn_state = ConnState::Handshaking;
        // Send handshake first if connection is outbound.
        if !inbound {
            self.send_handshake(&mut socket).await?;
        }

        tracing::trace!("waiting for handshake");
        // Receive handshake.
        if let Some(handshake) = socket.next().await {
            
            let handshake = handshake?;
            tracing::info!("read: handshake");
            tracing::debug!("handshake: {:#?}", handshake);
            
            // Validate handshake, and set ID.
            if handshake.protocol != PROTOCOL {
                return Err(PeerError::IncorrectProtocol);
            }
            if handshake.info_hash != self.torrent_ctx.info_hash {
                return Err(PeerError::IncorrectInfoHash);
            }
            self.id = Some(handshake.peer_id);

            // Respond with handshake if connection is inbound.
            if inbound {
                self.send_handshake(&mut socket).await?;
            }

            // Notify context that peer is connected.
            self.torrent_ctx.torrent_tx.send(torrent::CommandToTorrent::PeerConnected {
                address: self.address,
                id: handshake.peer_id,
            })?;

            tracing::info!("handshake successful, peer connected");
            Ok(socket.map_codec(|_| MessageCodec))
        
        } else {
            Err(PeerError::NoHandshake)
        }
    }

    async fn run(&mut self, socket: Framed<TcpStream, MessageCodec>) -> Result<()> {

        self.state.conn_state = ConnState::Introducing;
        let (mut sink, mut stream) = socket.split();
        let mut ticker = time::interval(time::Duration::from_secs(1));

        loop { tokio::select! {

            Some(msg) = stream.next() =>{
                let msg = msg?;
                tracing::info!("read: {}", msg);
                self.handle_msg(&mut sink, msg).await?;
            }

            Some(cmd) = self.peer_rx.recv() => {
                match cmd {

                    PeerCommand::BlockRead(block) => self.send_block(&mut sink, block).await?,

                    PeerCommand::PieceWritten(idx) => self.handle_written_piece(&mut sink, idx).await?,

                    PeerCommand::Shutdown => {
                        tracing::info!("session shutdown");
                        break;
                    },
                
                }
            }

            _ = ticker.tick() => self.tick().await?,

            // TODO: change this, i think its a timeout.
            else => {
                tracing::info!("else i think or smth {:?}", self.requests_out);
                for partial in self
                    .torrent_ctx
                    .picker
                    .partial_pieces
                    .read()
                    .await
                    .values()
                    .into_iter() 
                {
                    tracing::info!("{:?}", partial);
                }
                panic!();
            }

        }}

        Ok(())
    }

    async fn handle_msg(&mut self, sink: &mut MessageSink, msg: Message) -> Result<()> {

        match msg {
            // Bitfield can only be sent directly after handshake.
            Message::Bitfield(bitfiled) => {
                if self.state.conn_state == ConnState::Introducing {
                    self.handle_bitfield(sink, bitfiled).await?;
                } else {
                    tracing::error!("unexpected bitfield");
                    return Err(PeerError::UnexpectedBitfield);
                }
            },
            Message::KeepAlive => {},
            Message::Choke => {
                if !self.state.peer_choking {
                    self.state.peer_choking = true;
                    // Free any blocks in block requests out.
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
                if !self.state.peer_interested {
                    self.state.peer_interested = true;
                    self.send_message(sink, Message::Unchoke).await?;
                    self.state.choked = false;
                }
            },
            Message::NotInterested => {
                if self.state.peer_interested {
                    self.state.peer_interested = false;
                }
            },
            Message::Block(block) => {
                self.handle_block(block).await?;
                self.make_requests(sink).await?;
            },
            // TODO: do we need to stop whole task if request is invalid?
            // Will need to match error.
            Message::Request(block_info) => self.handle_request(block_info).await?,
            Message::Have { idx } => self.handle_have(sink, idx).await?,
            Message::Port { port: _ } => {},
            Message::Cancel(block_info) => self.handle_cancel(block_info).await?,
        }

        if self.state.conn_state == ConnState::Introducing {

            // Check if either us or peer has any pieces.
            if self.torrent_ctx.picker.piece_picker.read().await.own_bitfield().not_any()
            && self.bitfield.not_any() 
            {
                tracing::warn!("no pieces in connection");
                self.peer_tx.send(PeerCommand::Shutdown)?;
                return Ok(())
            }

            self.state.conn_state = ConnState::Connected;
        }

        Ok(())
    }

    async fn handle_bitfield(&mut self, sink: &mut MessageSink, mut bitfield: Bitfield) -> Result<()> {
        tracing::info!("peer has {}/{} pieces", bitfield.count_ones(), self.torrent_ctx.info.num_pieces);
        // Remove trailing bits.
        bitfield.resize(self.torrent_ctx.info.num_pieces as usize, false);
        // Interested if peer has pieces we don't.
        let interested = self.torrent_ctx.picker.piece_picker.write().await.bitfield_update(&bitfield);
        self.state.update(|state| state.num_pieces = bitfield.count_ones() as usize);
        self.bitfield = bitfield;
        self.update_interest(sink, interested).await
    }

    async fn handle_have(&mut self, sink: &mut MessageSink, idx: u32) -> Result<()> {
    
        if idx >= self.torrent_ctx.info.num_pieces {
            tracing::warn!("have msg with invalid idx: {}", idx);
            return Err(PeerError::InvalidMessage);
        }
        
        if self.bitfield[idx as usize] {
            return Ok(());
        }
        self.bitfield.set(idx as usize, true);
        self.state.update(|state| state.num_pieces += 1);

        let interested = self
            .torrent_ctx
            .picker
            .piece_picker
            .write()
            .await
            .increment_piece(idx as usize);

        self.update_interest(sink, interested).await
    }

    async fn handle_block(&mut self, block: block::Block) -> Result<()> {
        
        let block_info = block::BlockInfo {
            piece_idx: block.piece_idx,
            offset: block.offset,
            len: block.data.len(),
        };

        // Checks block validity and removes from requests_out.
        if !self.requests_out.remove(&block_info) {
            // TODO: penalise peer.
            // TODO: add defence against random block spamming.
            tracing::warn!("unexpected block: {:?}", &block_info);
            return Ok(());
        }
        
        let prev_block_state = if let Some(partial_piece) = self
            .torrent_ctx
            .picker
            .partial_pieces
            .read()
            .await
            .get(&block_info.piece_idx) 
        {
            partial_piece.write().await.received_block(&block_info)  
        } else {
            // This should'nt be possible.
            // Maybe it would in end game mode, if piece completed and already written.
            // Block is being checked for in requests_out, so it should be in partial_pieces.
            tracing::warn!("received block for non-existent piece: {:?}", &block_info);
            return Ok(());
        };

        if prev_block_state != BlockState::Received {
            self.state.update(|state| state.throughput.down += block.data.len() as u64);
            self.torrent_ctx.disk_tx
                .send(fs::CommandToDisk::WriteBlock { 
                    id: self.torrent_ctx.id,
                    block,
                })
                .map_err(|e| e.into())
                
        } else {
            // Again, do we need to check for spamming?
            // Should allow when in end game mode.
            tracing::warn!("duplicate block: {:?}", &block_info);
            Ok(())
        }
    }
    
    async fn handle_request(&mut self, block_info: block::BlockInfo) -> Result<()> {
        
        if self.state.choked {
            tracing::warn!("sending requests whilst choked");
            return Err(PeerError::InvalidMessage);
        }
        if !block_info.is_valid(&self.torrent_ctx.info) {
            tracing::warn!("invalid request: {:?}", block_info);
            return Err(PeerError::InvalidMessage);
        }
        if self.requests_out.contains(&block_info) {
            tracing::warn!("duplicate request: {:?}", block_info);
            return Ok(());
        }

        self.requests_out.insert(block_info);
        self.torrent_ctx.disk_tx.send(fs::CommandToDisk::ReadBlock {
            id: self.torrent_ctx.id,
            block: block_info,
            tx: self.peer_tx.clone(),
        })?;

        Ok(())
    }

    async fn handle_cancel(&mut self, block_info: block::BlockInfo) -> Result<()> {
        if !block_info.is_valid(&self.torrent_ctx.info) {
            tracing::warn!("invalid cancel: {:?}", block_info);
            return Err(PeerError::InvalidMessage);
        }
        self.requests_out.remove(&block_info);
        Ok(())
    }

    async fn handle_written_piece(
        &mut self,
        sink: &mut SplitSink<Framed<TcpStream, MessageCodec>, Message>,
        idx: usize,
    ) -> Result<()> {

        if !self.bitfield[idx] {
            sink.send(Message::Have { idx: idx as u32 }).await?;
        } else {
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
        
        // TODO: test whether quicker sending batch if requets.len() > 1.
        // let mut stream = futures::stream::iter(
        //     requests
        //         .into_iter()
        //         .map(|block| {
        //             tracing::info!("send request: {:?}", block);
        //             self.requests_out.insert(block);
        //             Ok(Message::Request(block))
        //         })
        // );
        // sink.send_all(&mut stream).await?;

        for block in requests {
            tracing::info!("send request: {:?}", block);
            self.requests_out.insert(block);
            sink.send(Message::Request(block)).await?;
        }

        Ok(())
    }

    async fn send_block(&mut self, sink: &mut MessageSink, block: block::Block) -> Result<()> {
        // TODO: just write a from/into for this.
        let block_info = block::BlockInfo {
            piece_idx: block.piece_idx,
            offset: block.offset,
            len: block.data.len(),
        };

        if !self.requests_in.remove(&block_info) {
            tracing::warn!("block read but no request: {:?}", block_info);
            return Ok(());
        }

        sink.send(Message::Block(block)).await?;
        self.state.update(|state| state.throughput.up += block_info.len as u64);

        Ok(())
    }

    async fn free_requests_out(&mut self) {
        tracing::info!("freeing requested blocks");
        let partial_pieces = self.torrent_ctx.picker.partial_pieces.read().await;
        for block in self.requests_out.drain() {
            if let Some(partial_piece) = partial_pieces.get(&block.piece_idx) {
                partial_piece.write().await.free_block(&block);
                tracing::trace!("freed block: {:?}", block);
            }
        }
    }
    
    // Send message to peer if we become interested.
    async fn update_interest(&mut self, sink: &mut MessageSink, interested: bool) -> Result<()> {
        // Become interested.
        if !self.state.interested && interested {
            self.state.interested = true;
            self.send_message(sink, Message::Interested).await?;
        } else if self.state.interested && !interested {
            self.state.interested = false;
        }
        Ok(())
    }

    async fn tick(&mut self) -> Result<()> {
        // TODO: Check for inactivity.

        if self.state.changed {
            self.torrent_ctx.torrent_tx.send(torrent::CommandToTorrent::PeerState {
                address: self.address,
                state: self.state,
            })?;
        }
        self.state.tick();
        tracing::debug!("{:#?}", self.state);

        Ok(())  
    }
}
