use std::{net::{Ipv4Addr, SocketAddr, ToSocketAddrs}, time::{Duration, Instant}};
use bytes::{Buf, BufMut, BytesMut};
use tokio::{net::UdpSocket, time};
use url::Url;
use super::{AnnounceParams, Event, Result, Tracker, TrackerError, DEFAULT_MIN_ANNOUNCE_INTERVAL};

// Reference: https://www.bittorrent.org/beps/bep_0015.html
// TODO: implement different chains of connect/announce based on circumstances.

const PROTOCOL_ID: i64      = 0x41727101980;
const ACTION_CONNECT: i32   = 0;
const ACTION_ANNOUNCE: i32  = 1;

pub struct UdpTracker {

    socket: UdpSocket,

    url: Url,

    conn_id: Option<i64>,

    last_announce: Option<Instant>,

    interval: Option<Duration>,

}

impl UdpTracker {

    pub async fn new(url: Url) -> Self {
        // Uses first available local port.
        let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        Self {
            socket,
            url,
            conn_id: None,
            last_announce: None,
            interval: None,
        }
    }

    async fn connect(&mut self) -> Result<()> {

        let host = self.url.host_str().ok_or(TrackerError::InvalidUrl)?;
        let port = self.url.port().ok_or(TrackerError::InvalidUrl)?;
        let addr = (host, port).to_socket_addrs()?.next().ok_or(TrackerError::InvalidUrl)?;
        let timeout_duration = Duration::from_secs(10);
        time::timeout(timeout_duration, self.socket.connect(addr)).await??;
        
        // Send connect request.
        let trans_id = rand::random();
        
        let mut buf = BytesMut::with_capacity(16);
        buf.put_i64(PROTOCOL_ID);
        buf.put_i32(ACTION_CONNECT);
        buf.put_i32(trans_id);
        
        self.socket.send(&buf).await?;
        
        // Receive connect response.
        let mut resp_buf = [0u8; 16];
        let n = self.socket.recv(&mut resp_buf).await?;
        if n < 16 {
            return Err(TrackerError::ResponseError("invalid response length".to_string()));
        }
        let mut resp = &resp_buf[..];
        if resp.get_i32() != ACTION_CONNECT {
            return Err(TrackerError::ResponseError("expected action 0".to_string()));
        }
        if resp.get_i32() != trans_id {
            return Err(TrackerError::ResponseError("invalid transaction id".to_string()));
        }
        self.conn_id = Some(resp.get_i64());
        
        tracing::trace!("connected to tracker");
        Ok(())
    }
}

#[async_trait::async_trait]
impl Tracker for UdpTracker {

    async fn announce(&mut self, params: AnnounceParams) -> Result<Vec<SocketAddr>> {
        
        self.connect().await?;

        let trans_id = rand::random();

        let mut buf = BytesMut::with_capacity(98);
        buf.put_i64(self.conn_id.unwrap());
        buf.put_i32(ACTION_ANNOUNCE);
        buf.put_i32(trans_id);
        buf.put(&params.info_hash[..]);
        buf.put(&params.client_id[..]);
        buf.put_u64(params.downloaded);
        buf.put_u64(params.left);
        buf.put_u64(params.uploaded);
        buf.put_i32(
            match params.event {
                Some(Event::Started) => 2,
                Some(Event::Completed) => 1,
                Some(Event::Stopped) => 3,
                None => 0,
            }
        );
        buf.put_i32(0); // IP address, default = 0.
        buf.put_i32(rand::random()); // Key, random.
        buf.put_i32(
            match params.num_want {
                Some(num_want) => num_want as i32,
                None => -1,
            }
        );
        buf.put_u16(params.port);

        self.socket.send(&buf).await?;

        let mut resp_buf = [0u8; 1024];
        let n = self.socket.recv(&mut resp_buf).await?;
        let mut resp = &resp_buf[..];
        if n < 20 {
            return Err(TrackerError::ResponseError("invalid response length".to_string()));
        }
        if resp.get_i32() != ACTION_ANNOUNCE {
            return Err(TrackerError::ResponseError("expected action 1".to_string()));
        }
        if resp.get_i32() != trans_id {
            return Err(TrackerError::ResponseError("invalid transaction id".to_string()));
        }
        let _interval = resp.get_i32();
        let _leechers = resp.get_i32();
        let _seeders = resp.get_i32();
        let num_peers = (n - 20) / 6;

        let mut peers = Vec::with_capacity(num_peers);
        for _ in 0..num_peers {
            let ip = resp.get_u32();
            let port = resp.get_u16();
            peers.push(SocketAddr::new(Ipv4Addr::from(ip).into(), port));
        }

        tracing::info!("provided {} peers", peers.len());
        self.last_announce = Some(Instant::now());
        Ok(peers)
    }

    fn can_announce(&self, time: Instant) -> bool {
        
        if let Some(last_announce) = self.last_announce {
            time.duration_since(last_announce) 
            >= self.interval.unwrap_or(Duration::from_secs(DEFAULT_MIN_ANNOUNCE_INTERVAL))
        
        } else {
            true
        }
    }

    fn should_announce(&self, time: Instant) -> bool { self.can_announce(time) }
}
