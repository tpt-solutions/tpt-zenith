// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Simulated constellation generator for synthetic TLE-like element sets.
//!
//! Produces Walker-style LEO constellations (Starlink/OneWeb-like) as
//! [`Tle`] objects so the rest of the engine can be exercised without live
//! NORAD data.

use crate::tle::Tle;

/// Parameters for a Walker-delta constellation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstellationSpec {
    /// Total number of satellites.
    pub total: usize,
    /// Number of orbital planes.
    pub planes: usize,
    /// Inclination, degrees.
    pub inclination_deg: f64,
    /// Altitude above mean Earth radius, kilometers.
    pub altitude_km: f64,
    /// Walker phasing parameter F (0..planes-1).
    pub phase_factor: usize,
    /// Mean motion (rev/day). Derived from altitude if zero.
    pub mean_motion: f64,
    /// Base international designator year for synthetic IDs.
    pub epoch_year: u32,
    /// Fractional day-of-year for the synthetic epoch.
    pub epoch_day: f64,
}

impl Default for ConstellationSpec {
    fn default() -> Self {
        ConstellationSpec {
            total: 60,
            planes: 6,
            inclination_deg: 53.0,
            altitude_km: 550.0,
            phase_factor: 1,
            mean_motion: 0.0,
            epoch_year: 2026,
            epoch_day: 1.0,
        }
    }
}

impl ConstellationSpec {
    /// Generate the synthetic TLE set.
    pub fn generate(&self) -> Vec<Tle> {
        let per_plane = self.total.div_ceil(self.planes);
        let mean_motion = if self.mean_motion > 0.0 {
            self.mean_motion
        } else {
            mean_motion_for_altitude(self.altitude_km)
        };

        let mut tles = Vec::with_capacity(self.total);
        let mut count = 0;
        for p in 0..self.planes {
            let raan = 360.0 * (p as f64) / (self.planes as f64);
            for s in 0..per_plane {
                if count >= self.total {
                    break;
                }
                let plane_offset = 360.0 * (s as f64) / (per_plane as f64);
                let phase_offset =
                    360.0 * (self.phase_factor as f64) * (s as f64) / (self.total as f64);
                let mean_anomaly = (plane_offset + phase_offset).rem_euclid(360.0);
                tles.push(self.make_tle(count, raan, mean_anomaly, mean_motion));
                count += 1;
            }
        }
        tles
    }

    fn make_tle(&self, index: usize, raan: f64, mean_anomaly: f64, mean_motion: f64) -> Tle {
        let yy = self.epoch_year % 100;
        let satnum = 90000 + index;
        Tle {
            name: Some(format!("ZENITH-{}-{}", self.planes, index)),
            satellite_number: satnum as u32,
            international_designator: format!("{:02}{:03}A", yy, index),
            epoch_year: self.epoch_year,
            epoch_day: self.epoch_day,
            mean_motion_dot: 0.0,
            mean_motion_ddot: 0.0,
            bstar: 0.0,
            element_set_number: 1,
            inclination_deg: self.inclination_deg,
            raan_deg: raan,
            eccentricity: 0.0001,
            arg_perigee_deg: 0.0,
            mean_anomaly_deg: mean_anomaly,
            mean_motion,
            rev_number_at_epoch: 1,
        }
    }
}

/// Mean motion (rev/day) for a circular orbit at `altitude_km` above the mean
/// Earth radius, via Kepler's third law.
pub fn mean_motion_for_altitude(altitude_km: f64) -> f64 {
    use crate::constants::{EARTH_RADIUS_KM, MU_EARTH};
    let a = EARTH_RADIUS_KM + altitude_km; // km
    let n_rad_s = (MU_EARTH / (a * a * a)).sqrt();
    n_rad_s * 86400.0 / (2.0 * std::f64::consts::PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_requested_count() {
        let spec = ConstellationSpec {
            total: 42,
            planes: 6,
            ..Default::default()
        };
        let tles = spec.generate();
        assert_eq!(tles.len(), 42);
    }

    #[test]
    fn altitude_gives_plausible_mean_motion() {
        // Starlink ~550 km -> ~15.05 rev/day.
        let mm = mean_motion_for_altitude(550.0);
        assert!((mm - 15.05).abs() < 0.2);
    }
}
