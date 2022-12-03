use std::{time, string::String};
use reqwest::Client;
use bytes::Bytes;

use crate::torrent;
use super::peer_parse::{BinaryModel, PeerInfo, ParsePeers, DictModel};
use super::{Result, Error};
use super::http_comms::{RequestParams, TrackerResponse};

pub struct Tracker {
    announce:   String,
    // A string the client should send to the tracker in its next request.
    id:         Option<String>,
    // http client.
    client:     Client,
    // Get request query parameters.
    params:     RequestParams,
    // How long client should wait before sending next request.
    interval:   Option<time::Duration>,
    // Time of last request.
    epoch:      time::Instant,
}

impl Tracker {

    pub fn new(torrent: &torrent::Torrent) -> Tracker {
        Tracker {
            announce:   torrent.announce().to_string(),
            id:         None,
            client:     Client::new(),
            params:     RequestParams::new(torrent),
            interval:   None,
            epoch:      time::Instant::now(),
        }
    }

    pub fn refresh_params(&mut self, uploaded: u64, downloaded: u64, left: u64) {
        self.params.refresh_params(uploaded, downloaded, left);
    }

    pub async fn request_peers(&mut self) -> Result<(Vec<PeerInfo>, u64, u64)> {
        let raw_resp = self.client.get(self.params.build_url(&self.id))
            .send()
            .await?
            .bytes()
            .await?;

        self.parse_response(raw_resp)
    }

    fn parse_response(&mut self, raw: Bytes) -> Result<(Vec<PeerInfo>, u64, u64)> {
        match bencode::decode_bytes(&raw) {
            // Success in parsing response with binary model.
            Ok(out) => self.handle_response::<BinaryModel>(out),
            // Try parsing response with dictionary model.
            Err(_) => {
                let resp: bencode::Result<TrackerResponse<DictModel>> = bencode::decode_bytes(&raw);
                match resp {
                    // Success in parsing response with dictionary model.
                    Ok(out) => self.handle_response::<DictModel>(out),
                    // Error in parsing response.
                    Err(e) => Err(Error::BencodeError(e)),
                }
            }
        }
    }

    fn handle_response<P: ParsePeers>(&mut self, resp: TrackerResponse<P>) -> Result<(Vec<PeerInfo>, u64, u64)> {
        // Check for failure.
        if let Some(reason) = resp.failure_reason {
            return Err(Error::TrackerError {
                msg: String::from_utf8_lossy(&reason).to_string(),
                code: resp.failure_code,
            });
        }

        if resp.min_interval.is_some() {
            self.interval = resp.min_interval.map(|i| time::Duration::from_secs(i));
        } else {
            self.interval = resp.interval.map(|i| time::Duration::from_secs(i));
        }

        self.epoch = time::Instant::now();
        self.id = resp.tracker_id.map(|id| String::from_utf8_lossy(&id).to_string());

        match resp.peers {
            Some(p) => {
                let peers = p.parse_peers(); 
                if peers.len() == 0 {
                    return Err(Error::TrackerError {
                        msg: "No peers found".to_string(),
                        code: None,
                    });
                } else {
                    Ok((peers, resp.complete.unwrap_or(0), resp.incomplete.unwrap_or(0)))
                }
            },
            None => Err(Error::TrackerError { code: None, msg: "Tracker sent no peers".to_string() }),
        }
    }
}