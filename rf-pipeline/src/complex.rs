// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A minimal complex baseband sample type, kept dependency-free like the rest
//! of the workspace.

use std::ops::{Add, Mul, Sub};

/// A complex baseband IQ sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    /// In-phase component.
    pub re: f64,
    /// Quadrature component.
    pub im: f64,
}

impl Complex {
    /// Construct a sample from its I/Q components.
    pub fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }

    /// A purely real sample (zero quadrature component).
    pub fn real(re: f64) -> Self {
        Complex { re, im: 0.0 }
    }

    /// Squared magnitude, `|z|^2`. Cheaper than [`Complex::norm`] when only
    /// relative distances matter (e.g. nearest-symbol decisions).
    pub fn norm_sqr(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Magnitude, `|z|`.
    pub fn norm(&self) -> f64 {
        self.norm_sqr().sqrt()
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, rhs: Complex) -> Complex {
        Complex::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, rhs: Complex) -> Complex {
        Complex::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul<f64> for Complex {
    type Output = Complex;
    fn mul(self, rhs: f64) -> Complex {
        Complex::new(self.re * rhs, self.im * rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_of_unit_samples() {
        assert!((Complex::new(1.0, 0.0).norm() - 1.0).abs() < 1e-12);
        assert!((Complex::new(0.0, 1.0).norm() - 1.0).abs() < 1e-12);
        assert!((Complex::new(3.0, 4.0).norm() - 5.0).abs() < 1e-12);
    }
}
