//! The xorshift128+ random number generator. Fast, and very random.

use rand::{Error, RngCore};

/// A stream of pseudo-random numbers generated using the xorshift+ technique
/// described here:
///
/// Vigna, Sebastiano (2014). "Further scramblings of Marsaglia's xorshift
/// generators". arXiv:1404.0390 (http://arxiv.org/abs/1404.0390)
///
/// That paper says:
///
///     In particular, we propose a tightly coded xorshift128+ generator that
///     does not fail systematically any test from the BigCrush suite of TestU01
///     (even reversed) and generates 64 pseudorandom bits in 1.10 ns on an
///     Intel(R) Core(TM) i7-4770 CPU @3.40GHz (Haswell). It is the fastest
///     generator we are aware of with such empirical statistical properties.
///
/// The stream of numbers produced by this method repeats every 2**128 - 1 calls
/// (i.e. never, for all practical purposes). Zero appears 2**64 - 1 times in
/// this period; all other numbers appear 2**64 times. Additionally, each *bit*
/// in the produced numbers repeats every 2**128 - 1 calls.
///
/// This generator is not suitable as a cryptographically secure random number
/// generator.
///
/// Unlike the RNGs in the rand crate, this one implements Hash and serde's
/// Serialize and Deserialize traits.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub struct XorShift128Plus {
    state: [u64; 2]
}

impl XorShift128Plus {
    pub fn new(seed: [u64; 2]) -> XorShift128Plus {
        XorShift128Plus { state: seed }
    }
}

impl RngCore for XorShift128Plus {
    fn next_u64(&mut self) -> u64 {
        let mut s1 = self.state[0];
        let s0 = self.state[1];
        self.state[0] = s0;
        s1 ^= s1 << 23;
        self.state[1] = s1 ^ s0 ^ (s1 >> 17) ^ (s0 >> 26);
        self.state[1].wrapping_add(s0)
    }

    fn next_u32(&mut self) -> u32 {
        (self.next_u64() & 0xffff_ffff) as u32
    }

    fn fill_bytes(&mut self, _dest: &mut [u8]) {
        unimplemented!("fill_bytes");
    }

    fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Error> {
        unimplemented!("try_fill_bytes");
    }
}

#[test]
fn simple() {
    let mut rng = XorShift128Plus::new([1, 4]);

    // Calculated by hand following the algorithm given in the paper. The upper
    // bits are mostly zero because we started with a poor seed; once it has run
    // for a while, we'll get an even mix of ones and zeros in all 64 bits.
    assert_eq!(rng.next_u64(), 0x800049);
    assert_eq!(rng.next_u64(), 0x3000186);
    assert_eq!(rng.next_u64(), 0x400003001145);
}
