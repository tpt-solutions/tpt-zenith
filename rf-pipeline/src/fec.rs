// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Forward error correction interfaces, plus a Hamming(7,4) single-error-
//! correcting block code. CCSDS-grade Reed-Solomon/convolutional/turbo coding
//! is not implemented; see `docs/ccsds-compliance.md` for what a
//! standards-track implementation would additionally need.

/// Encodes a bitstream by adding redundancy for error correction.
pub trait Encoder {
    /// Number of data bits consumed per codeword.
    fn data_bits(&self) -> usize;
    /// Number of coded bits produced per codeword.
    fn code_bits(&self) -> usize;

    /// Encode `bits` into codewords. If `bits.len()` is not a multiple of
    /// [`Encoder::data_bits`], the final block is padded with `false` bits.
    fn encode(&self, bits: &[bool]) -> Vec<bool>;
}

/// Recovers data bits from (possibly corrupted) codewords.
pub trait Decoder {
    /// Decode `bits` (a multiple of the codeword length) back into data bits,
    /// correcting errors where the code's distance allows.
    fn decode(&self, bits: &[bool]) -> Vec<bool>;
}

fn pad_to_multiple(bits: &[bool], n: usize) -> Vec<bool> {
    let mut v = bits.to_vec();
    let rem = v.len() % n;
    if rem != 0 {
        v.resize(v.len() + (n - rem), false);
    }
    v
}

/// Hamming(7,4): 4 data bits -> 7 coded bits, corrects any single-bit error
/// per codeword. Bit positions are 1-indexed per the classical construction
/// (`p1 p2 d1 p3 d2 d3 d4`), with parity covering positions `{1,3,5,7}`,
/// `{2,3,6,7}`, and `{4,5,6,7}` respectively.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hamming74;

impl Encoder for Hamming74 {
    fn data_bits(&self) -> usize {
        4
    }

    fn code_bits(&self) -> usize {
        7
    }

    fn encode(&self, bits: &[bool]) -> Vec<bool> {
        let padded = pad_to_multiple(bits, 4);
        padded
            .chunks(4)
            .flat_map(|d| {
                let (d1, d2, d3, d4) = (d[0], d[1], d[2], d[3]);
                let p1 = d1 ^ d2 ^ d4;
                let p2 = d1 ^ d3 ^ d4;
                let p3 = d2 ^ d3 ^ d4;
                [p1, p2, d1, p3, d2, d3, d4]
            })
            .collect()
    }
}

impl Decoder for Hamming74 {
    fn decode(&self, bits: &[bool]) -> Vec<bool> {
        bits.chunks(7)
            .filter(|c| c.len() == 7)
            .flat_map(|c| {
                // 1-indexed working copy: pos[0] is unused padding.
                let mut pos = [false; 8];
                pos[1..8].copy_from_slice(c);

                let s1 = pos[1] ^ pos[3] ^ pos[5] ^ pos[7];
                let s2 = pos[2] ^ pos[3] ^ pos[6] ^ pos[7];
                let s3 = pos[4] ^ pos[5] ^ pos[6] ^ pos[7];
                let syndrome = (s1 as usize) + (s2 as usize) * 2 + (s3 as usize) * 4;
                if syndrome != 0 {
                    pos[syndrome] = !pos[syndrome];
                }
                [pos[3], pos[5], pos[6], pos[7]]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_no_errors() {
        let bits = [true, false, true, true, false, false, true, false];
        let code = Hamming74;
        let encoded = code.encode(&bits);
        assert_eq!(encoded.len(), 14);
        let decoded = code.decode(&encoded);
        assert_eq!(decoded, bits);
    }

    #[test]
    fn corrects_any_single_bit_error_per_codeword() {
        let bits = [true, false, true, true];
        let code = Hamming74;
        let encoded = code.encode(&bits);
        for flip in 0..7 {
            let mut corrupted = encoded.clone();
            corrupted[flip] = !corrupted[flip];
            let decoded = code.decode(&corrupted);
            assert_eq!(decoded, bits, "failed to correct error at bit {flip}");
        }
    }

    #[test]
    fn pads_data_not_a_multiple_of_four() {
        let bits = [true, false, true]; // 3 bits -> padded to 4
        let code = Hamming74;
        let encoded = code.encode(&bits);
        assert_eq!(encoded.len(), 7);
        let decoded = code.decode(&encoded);
        assert_eq!(&decoded[..3], &bits);
    }
}
