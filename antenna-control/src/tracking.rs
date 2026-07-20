// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic tracking loop: orbital-mechanics pointing output to antenna
//! commands.
//!
//! A [`Tracker`] binds a propagated satellite to a ground station and produces
//! the topocentric look angles the dish must follow across a pass. The loop is
//! deterministic and clock-free: it advances a fixed simulated time step, so a
//! given TLE, station, and pass window always produce identical commands and
//! error statistics. Optional velocity feedforward ([`Tracker::with_lead`])
//! commands the antenna to where the satellite *will* be, cancelling most of the
//! actuator lag that would otherwise accumulate on a fast-moving pass.

use orbital_mechanics::error::Result;
use orbital_mechanics::visibility::{look_angles, GroundStation, LookAngles};
use orbital_mechanics::Propagator;
use std::time::Duration;

use crate::hal::{DishBackend, PointingCommand};

/// One recorded instant of a tracking run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackSample {
    /// Minutes after the TLE epoch at this sample.
    pub tsince_min: f64,
    /// True satellite azimuth at this instant, degrees.
    pub target_azimuth_deg: f64,
    /// True satellite elevation at this instant, degrees.
    pub target_elevation_deg: f64,
    /// Actual dish azimuth at this instant, degrees.
    pub actual_azimuth_deg: f64,
    /// Actual dish elevation at this instant, degrees.
    pub actual_elevation_deg: f64,
    /// Angular separation between the true and actual pointing, degrees.
    pub error_deg: f64,
}

/// Summary of a tracking run.
#[derive(Debug, Clone)]
pub struct TrackingReport {
    /// Per-tick samples over the tracked window.
    pub samples: Vec<TrackSample>,
    /// Largest pointing error observed, degrees.
    pub max_error_deg: f64,
    /// Root-mean-square pointing error, degrees.
    pub rms_error_deg: f64,
    /// Whether the dish acquired the target before the tracking phase began.
    pub acquired: bool,
}

/// Configuration for a tracking run.
#[derive(Debug, Clone, Copy)]
pub struct TrackConfig {
    /// Simulated control-loop period, seconds.
    pub tick_secs: f64,
    /// Angular tolerance for declaring acquisition, degrees.
    pub acquire_tolerance_deg: f64,
    /// Maximum simulated seconds allowed for the acquisition slew.
    pub acquire_timeout_secs: f64,
}

impl Default for TrackConfig {
    fn default() -> Self {
        TrackConfig {
            tick_secs: 0.5,
            acquire_tolerance_deg: 0.1,
            acquire_timeout_secs: 120.0,
        }
    }
}

/// Binds a satellite propagator to a ground station to drive an antenna.
#[derive(Debug, Clone)]
pub struct Tracker<'a> {
    sat: &'a Propagator,
    station: GroundStation,
    lead_min: f64,
}

impl<'a> Tracker<'a> {
    /// Create a tracker for `sat` observed from `station`, with no feedforward.
    pub fn new(sat: &'a Propagator, station: GroundStation) -> Self {
        Tracker {
            sat,
            station,
            lead_min: 0.0,
        }
    }

    /// Set a feedforward lead time (minutes): the antenna is commanded toward
    /// the satellite's predicted position `lead` minutes ahead of now.
    pub fn with_lead(mut self, lead_min: f64) -> Self {
        self.lead_min = lead_min;
        self
    }

    /// Compute the true look angles from the station to the satellite at
    /// `tsince_min` minutes after epoch.
    pub fn desired_pointing(&self, tsince_min: f64) -> Result<LookAngles> {
        let state = self.sat.propagate(tsince_min)?;
        let gmst = self.sat.gmst_rad(tsince_min);
        Ok(look_angles(&self.station, &state, gmst))
    }

    /// Track the satellite across `[aos_min, los_min]`, driving `backend`.
    ///
    /// The run has two phases: an acquisition slew that pre-positions the dish
    /// at the AOS look angle (not counted in the error statistics), followed by
    /// the tracking phase whose per-tick pointing error is recorded. Returns a
    /// [`TrackingReport`].
    pub fn track<B: DishBackend>(
        &self,
        backend: &mut B,
        aos_min: f64,
        los_min: f64,
        config: TrackConfig,
    ) -> Result<TrackingReport> {
        let dt = Duration::from_secs_f64(config.tick_secs);
        let dt_min = config.tick_secs / 60.0;

        // --- Acquisition phase: slew to the AOS look angle. ---
        let acquisition = self.desired_pointing(aos_min)?;
        // command() only fails on limit violations; a visible pass is above the
        // horizon, so treat a rejection as a non-fatal skip of acquisition.
        let _ = backend.command(PointingCommand::new(
            acquisition.azimuth_deg,
            acquisition.elevation_deg,
        ));
        let max_acquire_ticks = (config.acquire_timeout_secs / config.tick_secs).ceil() as usize;
        let mut acquired = false;
        for _ in 0..max_acquire_ticks {
            backend.tick(dt);
            let s = backend.state();
            let err = angular_separation_deg(
                s.azimuth_deg,
                s.elevation_deg,
                acquisition.azimuth_deg,
                acquisition.elevation_deg,
            );
            if err <= config.acquire_tolerance_deg {
                acquired = true;
                break;
            }
        }

        // --- Tracking phase. ---
        let mut samples = Vec::new();
        let mut sum_sq = 0.0;
        let mut max_error = 0.0_f64;

        let mut t = aos_min;
        while t <= los_min + 1e-9 {
            // Feedforward: command where the satellite will be next tick.
            let lead_t = (t + self.lead_min).min(los_min);
            let cmd = self.desired_pointing(lead_t)?;
            let _ = backend.command(PointingCommand::new(cmd.azimuth_deg, cmd.elevation_deg));

            backend.tick(dt);
            t += dt_min;

            // Compare where the dish now points against the true position now.
            let truth = self.desired_pointing(t.min(los_min))?;
            let s = backend.state();
            let error = angular_separation_deg(
                s.azimuth_deg,
                s.elevation_deg,
                truth.azimuth_deg,
                truth.elevation_deg,
            );
            sum_sq += error * error;
            max_error = max_error.max(error);
            samples.push(TrackSample {
                tsince_min: t.min(los_min),
                target_azimuth_deg: truth.azimuth_deg,
                target_elevation_deg: truth.elevation_deg,
                actual_azimuth_deg: s.azimuth_deg,
                actual_elevation_deg: s.elevation_deg,
                error_deg: error,
            });
        }

        let rms = if samples.is_empty() {
            0.0
        } else {
            (sum_sq / samples.len() as f64).sqrt()
        };

        Ok(TrackingReport {
            samples,
            max_error_deg: max_error,
            rms_error_deg: rms,
            acquired,
        })
    }
}

/// Angular separation, in degrees, between two topocentric pointing directions
/// given as azimuth/elevation pairs. Correctly accounts for the convergence of
/// azimuth lines near zenith, so it is a true angular error rather than a naive
/// coordinate difference.
pub fn angular_separation_deg(az1_deg: f64, el1_deg: f64, az2_deg: f64, el2_deg: f64) -> f64 {
    let v1 = unit_vector(az1_deg, el1_deg);
    let v2 = unit_vector(az2_deg, el2_deg);
    let dot = (v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]).clamp(-1.0, 1.0);
    dot.acos().to_degrees()
}

/// Unit pointing vector in a local east-north-up frame.
fn unit_vector(az_deg: f64, el_deg: f64) -> [f64; 3] {
    let az = az_deg.to_radians();
    let el = el_deg.to_radians();
    let (sin_az, cos_az) = az.sin_cos();
    let (sin_el, cos_el) = el.sin_cos();
    [cos_el * sin_az, cos_el * cos_az, sin_el]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separation_zero_for_identical() {
        assert!(angular_separation_deg(123.0, 45.0, 123.0, 45.0) < 1e-9);
    }

    #[test]
    fn separation_ninety_degrees() {
        // Horizon north vs zenith are 90 deg apart.
        let sep = angular_separation_deg(0.0, 0.0, 0.0, 90.0);
        assert!((sep - 90.0).abs() < 1e-9, "sep {sep}");
    }

    #[test]
    fn separation_handles_azimuth_wrap() {
        // Two points near the horizon 20 deg apart in azimuth across 0/360.
        let sep = angular_separation_deg(350.0, 0.0, 10.0, 0.0);
        assert!((sep - 20.0).abs() < 1e-9, "sep {sep}");
    }
}
