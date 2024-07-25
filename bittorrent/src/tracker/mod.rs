use std::{net::SocketAddr, time::Instant};
use tokio::task::JoinHandle;
use tracing::Instrument;
use url::Url;
use crate::{torrent::{TorrentCommand, TorrentTx}, ID};

mod http;
mod udp;
pub use udp::UdpTracker;
pub use http::HttpTracker;

type Result<T> = std::result::Result<T, TrackerError>;
pub type TrackerTx = tokio::sync::watch::Sender<Option<AnnounceParams>>;
pub type TrackerRx = tokio::sync::watch::Receiver<Option<AnnounceParams>>;

// In cases where the tracker doesn't give us a min interval.
const DEFAULT_MIN_ANNOUNCE_INTERVAL: u64 = 60; // seconds

#[derive(thiserror::Error, Debug)]
pub enum TrackerError {

    #[error("request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("error deserializing response: {0}")]
    BencodeError(#[from]bencode::Error),

    #[error("timeout")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("invalid url")]
    InvalidUrl,
    
    #[error("response error: {0}")]
    ResponseError(String),

}

pub struct TrackersHandle {

    // Bit wasteful to keep here i guess.
    urls: Vec<Url>,

    handles: Vec<JoinHandle<()>>,

    tracker_rx: TrackerRx,

    pub tracker_tx: TrackerTx,
    
}

impl TrackersHandle {

    pub fn new(urls: Vec<Vec<Url>>) -> Self {
        
        let (tracker_tx, tracker_rx) = tokio::sync::watch::channel(None);
        let urls = urls.into_iter().flatten().collect();

        Self {
            urls,
            tracker_rx,
            tracker_tx,
            handles: Vec::new(),
        }
    }

    pub async fn start(&mut self, torrent_tx: TorrentTx) {
        
        let mut handles = vec![];
        for url in self.urls.iter() {

            // Create tracker based on scheme.
            let mut tracker: Box<dyn Tracker> = match url.scheme() {
                "http" => Box::new(HttpTracker::new(url.clone())),
                "udp"  => Box::new(UdpTracker::new(url.clone()).await),
                _ => {
                    tracing::warn!("unsupported tracker scheme: {}", url.scheme());
                    continue;
                },
            };

            let tx = torrent_tx.clone();
            let rx = self.tracker_rx.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = tracker.run(tx, rx).await {
                    tracing::error!("tracker error: {}", e);
                }
            }.instrument(tracing::info_span!("tracker", url = %url)));
            handles.push(handle);
        }

        self.handles = handles;
    }

    pub async fn shutdown(&mut self) {
        for handle in self.handles.drain(..) {
            if let Err(e) = handle.await {
                tracing::error!("tracker join error: {}", e);
            };
        }
    }
}

#[async_trait::async_trait]
pub trait Tracker: Send + Sync {

    async fn announce(&mut self, params: AnnounceParams) -> Result<Vec<SocketAddr>>;

    fn can_announce(&self, time: Instant) -> bool;

    fn should_announce(&self, time: Instant) -> bool;

    async fn run(
        &mut self,
        torrent_tx: TorrentTx,
        mut tracker_rx: TrackerRx,
    ) -> Result<()> {
        loop {

            tracker_rx.changed().await.ok();
            let params = *tracker_rx.borrow();
            let time = Instant::now();

            if let Some(params) = params {
                if params.event.is_some()
                || (params.num_want > Some(0) && self.can_announce(time))
                || self.should_announce(time) {

                    let peers = self.announce(params).await?;
                    if torrent_tx.send(TorrentCommand::Peers(peers)).is_err() {
                        return Ok(());
                    }
                
                }
            }

        }
    }

}

#[derive(Debug, Copy, Clone, Default)]
pub struct AnnounceParams {
    
    // Hash of info dict.
    pub info_hash:  ID,
    
    // Urlencoded 20-byte string used as a unique ID for the client.
    pub client_id:    ID,
    
    // Port number.
    pub port:       u16,
    
    // The total amount uploaded (since the client sent the 'started' event to the tracker) in base ten ASCII..
    pub uploaded:   u64,
    
    // The total amount downloaded (since the client sent the 'started' event to the tracker) in base ten ASCII..
    pub downloaded: u64,
    
    // The number of bytes this client still has to download in base ten ASCII. 
    // Clarification: The number of bytes needed to download to be 100% complete and get all the included files in the torrent.
    pub left:       u64,
    
    // If specified, must be one of started, completed, stopped, (or empty which is the same as not being specified). 
    // If not specified, then this request is one performed at regular intervals.
    pub event:     Option<Event>,
    
    // Number of peers that the client would like to receive from the tracker.
    pub num_want: Option<usize>,

}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum Event {

    Completed,
    
    #[default]
    Started,

    Stopped,

}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Completed => write!(f, "completed"),
            Event::Started => write!(f, "started"),
            Event::Stopped => write!(f, "stopped"),
        }
    }
}