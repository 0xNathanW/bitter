/*The bitfield message is variable length, where X is the length of the bitfield.
The payload is a bitfield representing the pieces that have been successfully downloaded.
The high bit in the first byte corresponds to piece index 0.
Bits that are cleared indicated a missing piece, and set bits indicate a valid and available piece.
Spare bits at the end are set to zero.*/

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitfield(pub Vec<u8>);

impl Bitfield {

    pub fn new(size: usize) -> Self {
        Self(vec![0; size])
    }

    pub fn has_piece(&self, idx: usize) -> bool {
        if idx >= self.0.len() * 8 {
            return false;
        }
        let byte = self.0[idx / 8];
        let bit = 7 - (idx % 8);
        byte & (1 << bit) != 0
    }

    pub fn set_piece(&mut self, idx: usize) {
        if idx >= self.0.len() * 8 {
            return;
        }
        let byte = idx / 8;
        let bit = 7 - (idx % 8);
        self.0[byte] |= 1 << bit;
    }
}

impl From<Vec<u8>> for Bitfield {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

#[cfg(test)]
mod tests {
    use sha1::digest::typenum::bit;

    use super::*;

    #[test]
    fn test_bitfield() {
        let mut bitfield = Bitfield::new(10);
        assert_eq!(bitfield.0.len(), 10);

        assert_eq!(bitfield.has_piece(0), false);
        bitfield.set_piece(0);
        assert_eq!(bitfield.has_piece(0), true);
    
        assert_eq!(bitfield.has_piece(77), false);
        bitfield.set_piece(77);
        assert_eq!(bitfield.has_piece(77), true);
        bitfield.set_piece(77);
        assert_eq!(bitfield.has_piece(77), true);
    }
}