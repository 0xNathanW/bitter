use crate::stats::ThroughputStats;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ConnState {
    Connecting,
    Connected,
    Disconnected,
    Handshaking,
    Introducing, // Where peers tell each other what pieces they have.
}

#[derive(Debug, Clone, Copy)]
pub struct SessionState {

    pub conn_state: ConnState,

    // Whether we are answering the peer's requests.
    pub choked: bool,

    // Whether we are interested in the peer's pieces.
    pub interested: bool,

    // Whether the peer is answering our requests.
    pub peer_choking: bool,

    // Whether the peer is interested in our pieces.
    pub peer_interested: bool,

    pub throughput: ThroughputStats,

    pub num_pieces: usize,

    pub changed: bool,

}

impl Default for SessionState {
    fn default() -> SessionState {
        SessionState {
            conn_state: ConnState::Disconnected,
            choked: true,
            interested: false,
            peer_choking: true,
            peer_interested: false,
            throughput: ThroughputStats::default(),
            changed: false,
            num_pieces: 0,
        }
    }
}

impl SessionState {

    pub fn tick(&mut self) {
        self.throughput.reset();
    }

    #[inline(always)]
    pub fn update(&mut self, f: impl FnOnce(&mut SessionState)) {
        f(self);
        self.changed = true;
    }
}