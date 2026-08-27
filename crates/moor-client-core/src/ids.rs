//! Id minting without a clock or an RNG. The host supplies entropy once
//! (`IdSeed`) and the current time on every `Input::Tick`; ids are ULIDs of
//! that time plus a counter-mixed hash of the seed.

use moor_protocol::{CommentId, ReviewId};

/// 128 bits of host-supplied entropy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdSeed(pub u128);

#[derive(Debug)]
pub(crate) struct IdGen {
    seed: u128,
    counter: u64,
}

impl IdGen {
    pub(crate) fn new(seed: IdSeed) -> Self {
        Self {
            seed: seed.0,
            counter: 0,
        }
    }

    pub(crate) fn comment_id(&mut self, now_ms: u64) -> CommentId {
        self.counter += 1;
        CommentId::from_parts(now_ms, self.random())
    }

    pub(crate) fn review_id(&mut self, now_ms: u64) -> ReviewId {
        self.counter += 1;
        ReviewId::from_parts(now_ms, self.random())
    }

    /// `splitmix64` over the seed and counter, two lanes for 128 bits; the
    /// ULID keeps the low 80.
    fn random(&self) -> u128 {
        let (seed_lo, seed_hi) = lanes(self.seed.to_le_bytes());
        let lo = splitmix(seed_lo ^ self.counter);
        let hi = splitmix(seed_hi ^ self.counter.rotate_left(32));
        (u128::from(hi) << 64) | u128::from(lo)
    }
}

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Split 16 little-endian bytes into two `u64` lanes.
fn lanes(b: [u8; 16]) -> (u64, u64) {
    let mut lo = [0u8; 8];
    let mut hi = [0u8; 8];
    lo.copy_from_slice(&b[..8]);
    hi.copy_from_slice(&b[8..]);
    (u64::from_le_bytes(lo), u64::from_le_bytes(hi))
}
