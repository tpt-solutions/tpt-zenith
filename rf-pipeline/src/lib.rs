// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! RF Signal Processing Pipeline (Phase 4, simulation-only).
//!
//! Provides a modem (modulation/demodulation), forward error correction, a
//! simulated AWGN channel, and CCSDS-style Space Packet framing, so the
//! modem/FEC chain can be designed and validated entirely in software before
//! any real SDR hardware (HackRF/USRP, Phase 7) is involved.
//!
//! See `docs/gnu-radio-vs-rust-decision.md` for the SDR-framework spike and
//! `docs/ccsds-compliance.md` for how this maps to the CCSDS standards this
//! crate is inspired by (and what it deliberately doesn't implement yet).

pub mod channel;
pub mod complex;
pub mod error;
pub mod fec;
pub mod frame;
pub mod modem;

pub use crate::channel::AwgnChannel;
pub use crate::complex::Complex;
pub use crate::fec::{Decoder, Encoder, Hamming74};
pub use crate::frame::{PacketType, SequenceFlags, SpacePacket, SpacePacketHeader};
pub use crate::modem::{Bpsk, Demodulator, Modulator, Qpsk};

/// Run `bits` through the full simulated link: FEC encode, modulate, the
/// (possibly noisy) channel, demodulate, then FEC decode.
pub fn simulate_link<M, C>(
    bits: &[bool],
    modem: &M,
    code: &C,
    channel: &mut AwgnChannel,
) -> Vec<bool>
where
    M: Modulator + Demodulator,
    C: Encoder + Decoder,
{
    let coded = code.encode(bits);
    let symbols = modem.modulate(&coded);
    let noisy = channel.transmit(&symbols);
    let demapped = modem.demodulate(&noisy);
    code.decode(&demapped)
}

/// Fraction of positions where `a` and `b` disagree, over the length of the
/// shorter of the two (trailing padding from block codes/modems is ignored
/// by comparing only up to `a.len().min(b.len())`).
pub fn bit_error_rate(a: &[bool], b: &[bool]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let errors = a
        .iter()
        .zip(b.iter())
        .take(n)
        .filter(|(x, y)| x != y)
        .count();
    errors as f64 / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::Rng;

    fn random_bits(n: usize, seed: u64) -> Vec<bool> {
        let mut rng = Rng::new(seed);
        (0..n).map(|_| rng.next_f64() < 0.5).collect()
    }

    #[test]
    fn link_is_lossless_with_no_noise() {
        let bits = random_bits(400, 1);
        let mut channel = AwgnChannel::new(0.0, 2);
        let recovered = simulate_link(&bits, &Qpsk, &Hamming74, &mut channel);
        assert_eq!(bit_error_rate(&bits, &recovered), 0.0);
    }

    #[test]
    fn fec_reduces_bit_error_rate_on_a_noisy_channel() {
        let bits = random_bits(4000, 3);

        // Same noise realization (fixed seed) for both the coded and
        // uncoded path, so the comparison isolates the effect of FEC.
        let noise_std = 0.55;
        let mut coded_channel = AwgnChannel::new(noise_std, 99);
        let coded_recovered = simulate_link(&bits, &Qpsk, &Hamming74, &mut coded_channel);
        let coded_ber = bit_error_rate(&bits, &coded_recovered);

        let mut uncoded_channel = AwgnChannel::new(noise_std, 99);
        let symbols = Qpsk.modulate(&bits);
        let noisy = uncoded_channel.transmit(&symbols);
        let uncoded_recovered = Qpsk.demodulate(&noisy);
        let uncoded_ber = bit_error_rate(&bits, &uncoded_recovered);

        assert!(
            coded_ber < uncoded_ber,
            "expected FEC to help: coded BER {coded_ber}, uncoded BER {uncoded_ber}"
        );
    }

    #[test]
    fn bit_error_rate_counts_mismatches() {
        let a = [true, false, true, true];
        let b = [true, true, true, false];
        assert!((bit_error_rate(&a, &b) - 0.5).abs() < 1e-12);
        assert_eq!(bit_error_rate(&a, &a), 0.0);
    }
}
