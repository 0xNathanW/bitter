
#[derive(Debug, PartialEq)]
pub enum ConnState {
    Connecting,
    Connected,
    Disconnected,
    Handshaking,
    Introducing, // Where peers tell each other what pieces they have.
}

#[derive(Debug)]
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

}

impl Default for SessionState {
    fn default() -> SessionState {
        SessionState {
            conn_state: ConnState::Disconnected,
            choked: true,
            interested: false,
            peer_choking: true,
            peer_interested: false,
        }
    }
}