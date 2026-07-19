// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hardware abstraction layer with a simulated dish backend (stub).

/// A hardware backend capable of receiving pointing commands and reporting
/// the dish's actual orientation. A real microcontroller backend can implement
/// this same trait to replace the simulation.
pub trait DishBackend {
    /// Command the dish to point at the given azimuth/elevation (degrees).
    fn point_to(&mut self, azimuth_deg: f64, elevation_deg: f64);
    /// Report the dish's current azimuth/elevation (degrees).
    fn current_pointing(&self) -> (f64, f64);
}
