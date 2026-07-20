// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A simulated additive white Gaussian noise (AWGN) channel, plus a small
//! deterministic PRNG so link simulations are reproducible without pulling in
//! an external `rand` dependency.

use crate::complex::Complex;

/// A tiny xorshift64* PRNG. Not cryptographically secure; only used to drive
/// reproducible noise in link simulations.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Seed the generator. A zero seed is remapped since xorshift cannot
    /// escape the all-zero state.
    pub fn new(seed: u64) -> Self {
        Rng {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }

    /// Next raw 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform sample in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A standard-normal sample via the Box-Muller transform.
    pub fn next_gaussian(&mut self) -> f64 {
        // Avoid u1 == 0.0 (ln(0) is undefined).
        let u1 = (1.0 - self.next_f64()).max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// A complex AWGN channel: adds independent Gaussian noise to each of a
/// symbol's I and Q components.
#[derive(Debug, Clone)]
pub struct AwgnChannel {
    /// Noise standard deviation applied independently to I and Q.
    pub noise_std: f64,
    rng: Rng,
}

impl AwgnChannel {
    /// A channel with an explicit per-dimension noise standard deviation.
    pub fn new(noise_std: f64, seed: u64) -> Self {
        AwgnChannel {
            noise_std,
            rng: Rng::new(seed),
        }
    }

    /// A channel targeting a given `Es/N0` in dB, for unit-energy symbols
    /// (matching [`crate::modem::Bpsk`]/[`crate::modem::Qpsk`]). Noise energy
    /// `N0` is split evenly across the I and Q dimensions.
    pub fn from_es_n0_db(es_n0_db: f64, seed: u64) -> Self {
        let es_n0 = 10f64.powf(es_n0_db / 10.0);
        let n0 = 1.0 / es_n0; // Es = 1 for unit-energy symbols.
        AwgnChannel::new((n0 / 2.0).sqrt(), seed)
    }

    /// Pass `symbols` through the channel, returning noisy symbols.
    pub fn transmit(&mut self, symbols: &[Complex]) -> Vec<Complex> {
        symbols
            .iter()
            .map(|s| {
                let noise = Complex::new(
                    self.noise_std * self.rng.next_gaussian(),
                    self.noise_std * self.rng.next_gaussian(),
                );
                *s + noise
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_noise_is_transparent() {
        let mut ch = AwgnChannel::new(0.0, 1);
        let symbols = [Complex::new(1.0, 0.0), Complex::new(-1.0, 0.0)];
        let out = ch.transmit(&symbols);
        assert_eq!(out, symbols);
    }

    #[test]
    fn gaussian_samples_are_roughly_standard_normal() {
        let mut rng = Rng::new(42);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.next_gaussian()).collect();
        let mean: f64 = samples.iter().sum::<f64>() / n as f64;
        let var: f64 = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var - 1.0).abs() < 0.1, "var {var}");
    }

    #[test]
    fn same_seed_is_reproducible() {
        let mut a = AwgnChannel::new(0.5, 7);
        let mut b = AwgnChannel::new(0.5, 7);
        let symbols = [Complex::new(1.0, 0.0); 10];
        assert_eq!(a.transmit(&symbols), b.transmit(&symbols));
    }
}
