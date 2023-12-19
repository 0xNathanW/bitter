use crate::Bitfield;

/*
A better strategy is to download pieces in rarest first order. The client can determine this 
by keeping the initial bitfield from each peer, and updating it with every have message. 
Then, the client can download the pieces that appear least frequently in these peer bitfields. 
Note that any Rarest First strategy should include randomization among at least several of the 
least common pieces, as having many clients all attempting to jump on the same "least common" 
piece would be counter productive
*/ 

#[derive(Clone, Copy, Default, Debug)]
struct PieceInfo {
    frequency: usize,
    is_pending: bool,
}

#[derive(Debug)]
pub struct PieceSelector {

    // Tracks frequency of all pieces.
    pieces: Vec<PieceInfo>,

    // Tracks pieces the client already has.
    own_bitfield: Bitfield,

    // Number of pieces not yet recieved may be pending.
    missing: usize,

    // Number of pieces available to pick.
    free: usize,

}

impl PieceSelector {

    pub fn new(num_pieces: usize) -> PieceSelector {
        PieceSelector {
            pieces: vec![PieceInfo::default(); num_pieces],
            own_bitfield: Bitfield::with_capacity(num_pieces),
            missing: num_pieces,
            free: num_pieces,
        }
    }

    pub fn own_bitfield(&self) -> &Bitfield {
        &self.own_bitfield
    }

    pub fn all_picked(&self) -> bool {
        self.free == 0
    }

    // After a "have" msg increment piece freq, returns if we are interested in the piece.
    pub fn update_piece_availability(&mut self, idx: usize) -> bool {
        self.pieces[idx].frequency += 1;
        *self.own_bitfield.get(idx).unwrap()
    }

    // Updates pieces freq when given entire bitfield, returns interested if they have at least one piece we don't.
    pub fn bitfield_update(&mut self, bitfield: &Bitfield) -> bool {
        debug_assert!(self.own_bitfield.len() == bitfield.len());
        let mut interested = false;
        for idx in 0..bitfield.len() {
            if *bitfield.get(idx).unwrap() {
                self.pieces[idx].frequency += 1;
                if !interested && !self.own_bitfield.get(idx).unwrap() {
                    interested = true;
                }
            }
        }
        interested
    }

    pub fn add_downloaded_piece(&mut self, idx: usize) {
        debug_assert!(!self.own_bitfield.get(idx).unwrap());
        self.own_bitfield.set(idx, true);
        self.missing -= 1;
    }

    // TODO: update to rarest.
    pub fn select_piece(&mut self) -> Option<usize> {
        
        for idx in 0..self.own_bitfield.len() {
            if !self.own_bitfield.get(idx).unwrap() && self.pieces[idx].frequency != 0 && !self.pieces[idx].is_pending {

                self.pieces[idx].is_pending = true;
                self.free -= 1;
                return Some(idx)
            }
        }

        None
    }
}