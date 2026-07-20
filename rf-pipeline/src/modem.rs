// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Modulation/demodulation interfaces and two baseband modem implementations
//! (BPSK, QPSK). All symbols are normalized to unit energy so a single
//! [`crate::channel::AwgnChannel`] noise variance is meaningful across modems.

use crate::complex::Complex;

/// Maps a bitstream to complex baseband symbols.
pub trait Modulator {
    /// Number of bits carried by each symbol.
    fn bits_per_symbol(&self) -> usize;

    /// Modulate `bits` into IQ symbols. If `bits.len()` is not a multiple of
    /// [`Modulator::bits_per_symbol`], the final symbol is padded with `false`
    /// bits (callers that care should track the original bit count and trim
    /// after demodulation).
    fn modulate(&self, bits: &[bool]) -> Vec<Complex>;
}

/// Recovers a bitstream from (possibly noisy) complex baseband symbols via
/// hard-decision demapping.
pub trait Demodulator {
    /// Demodulate `symbols` back into bits (`bits_per_symbol` bits each).
    fn demodulate(&self, symbols: &[Complex]) -> Vec<bool>;
}

fn pad_to_multiple(bits: &[bool], n: usize) -> Vec<bool> {
    let mut v = bits.to_vec();
    let rem = v.len() % n;
    if rem != 0 {
        v.resize(v.len() + (n - rem), false);
    }
    v
}

/// Binary Phase Shift Keying: one bit per symbol on the real axis.
/// `false -> +1`, `true -> -1`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bpsk;

impl Modulator for Bpsk {
    fn bits_per_symbol(&self) -> usize {
        1
    }

    fn modulate(&self, bits: &[bool]) -> Vec<Complex> {
        bits.iter()
            .map(|&b| Complex::real(if b { -1.0 } else { 1.0 }))
            .collect()
    }
}

impl Demodulator for Bpsk {
    fn demodulate(&self, symbols: &[Complex]) -> Vec<bool> {
        symbols.iter().map(|s| s.re < 0.0).collect()
    }
}

/// Quadrature Phase Shift Keying: two Gray-coded bits per symbol, unit
/// energy (`I, Q in {+-1/sqrt(2)}`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Qpsk;

const INV_SQRT2: f64 = std::f64::consts::FRAC_1_SQRT_2;

impl Modulator for Qpsk {
    fn bits_per_symbol(&self) -> usize {
        2
    }

    fn modulate(&self, bits: &[bool]) -> Vec<Complex> {
        let padded = pad_to_multiple(bits, 2);
        padded
            .chunks(2)
            .map(|c| {
                let i = if c[0] { -INV_SQRT2 } else { INV_SQRT2 };
                let q = if c[1] { -INV_SQRT2 } else { INV_SQRT2 };
                Complex::new(i, q)
            })
            .collect()
    }
}

impl Demodulator for Qpsk {
    fn demodulate(&self, symbols: &[Complex]) -> Vec<bool> {
        symbols
            .iter()
            .flat_map(|s| [s.re < 0.0, s.im < 0.0])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bpsk_round_trip_no_noise() {
        let bits = [true, false, true, true, false];
        let m = Bpsk;
        let symbols = m.modulate(&bits);
        let recovered = m.demodulate(&symbols);
        assert_eq!(recovered, bits);
    }

    #[test]
    fn qpsk_round_trip_no_noise() {
        let bits = [true, false, true, true, false, false, true, false];
        let m = Qpsk;
        let symbols = m.modulate(&bits);
        assert_eq!(symbols.len(), bits.len() / 2);
        let recovered = m.demodulate(&symbols);
        assert_eq!(recovered, bits);
    }

    #[test]
    fn qpsk_pads_odd_length_input() {
        let bits = [true, false, true];
        let m = Qpsk;
        let symbols = m.modulate(&bits);
        assert_eq!(symbols.len(), 2); // 3 bits -> padded to 4 -> 2 symbols
        let recovered = m.demodulate(&symbols);
        assert_eq!(&recovered[..3], &bits);
    }

    #[test]
    fn symbols_have_unit_energy() {
        for s in Bpsk.modulate(&[true, false]) {
            assert!((s.norm() - 1.0).abs() < 1e-12);
        }
        for s in Qpsk.modulate(&[true, false, true, true]) {
            assert!((s.norm() - 1.0).abs() < 1e-12);
        }
    }
}
