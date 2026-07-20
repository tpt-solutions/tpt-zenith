// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hardware abstraction layer (HAL) for antenna dishes.
//!
//! The [`DishBackend`] trait is the interface contract between the deterministic
//! tracking loop ([`crate::tracking`]) and a physical (or simulated) antenna
//! positioner. A single trait is implemented by both the [`SimulatedDish`] used
//! for software-only testing today and, in Phase 7, by a real microcontroller
//! backend driving azimuth/elevation motors. See the module-level notes in
//! [`crate`] and the "Interface contract" section below for the guarantees a
//! conforming backend must uphold.
//!
//! # Coordinate conventions
//!
//! All angles are in degrees. Azimuth is measured clockwise from true north in
//! `[0, 360)`. Elevation is measured up from the local horizon in `[0, 90]`.
//! These match the topocentric look angles produced by the orbital-mechanics
//! visibility module.
//!
//! # Interface contract
//!
//! A conforming [`DishBackend`] must guarantee:
//!
//! 1. **Command validation.** [`DishBackend::command`] rejects any target
//!    outside the backend's [`DishLimits`] with a [`DishError`], leaving the
//!    current setpoint unchanged.
//! 2. **Monotonic actuation.** After a valid `command`, repeated calls to
//!    [`DishBackend::tick`] move the reported [`DishState`] toward the setpoint,
//!    never exceeding the advertised slew rates, and converge in finite time.
//! 3. **Time semantics.** `tick(dt)` advances the actuator by `dt` of simulated
//!    time. A real backend that moves autonomously in wall-clock time may treat
//!    `tick` as a servicing/watchdog hook and ignore `dt`, but it must still
//!    report an up-to-date [`DishState`] from [`DishBackend::state`].
//! 4. **Angle normalization.** Reported azimuth is always in `[0, 360)` and
//!    elevation in `[limits.min_elevation_deg, limits.max_elevation_deg]`.

use std::fmt;
use std::time::Duration;

/// A commanded pointing setpoint in local topocentric coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointingCommand {
    /// Target azimuth, degrees clockwise from true north.
    pub azimuth_deg: f64,
    /// Target elevation, degrees above the local horizon.
    pub elevation_deg: f64,
}

impl PointingCommand {
    /// Construct a pointing command.
    pub fn new(azimuth_deg: f64, elevation_deg: f64) -> Self {
        PointingCommand {
            azimuth_deg,
            elevation_deg,
        }
    }
}

/// The reported state of a dish positioner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DishState {
    /// Current azimuth, degrees clockwise from true north in `[0, 360)`.
    pub azimuth_deg: f64,
    /// Current elevation, degrees above the local horizon.
    pub elevation_deg: f64,
    /// Whether the dish is still slewing toward its setpoint.
    pub slewing: bool,
}

/// Mechanical envelope and rate limits advertised by a backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DishLimits {
    /// Maximum azimuth slew rate, degrees per second.
    pub max_azimuth_rate_deg_s: f64,
    /// Maximum elevation slew rate, degrees per second.
    pub max_elevation_rate_deg_s: f64,
    /// Lowest commandable elevation, degrees (mechanical horizon).
    pub min_elevation_deg: f64,
    /// Highest commandable elevation, degrees (typically 90 = zenith).
    pub max_elevation_deg: f64,
}

impl Default for DishLimits {
    fn default() -> Self {
        // A capable mid-size tracking dish: several degrees per second, full
        // hemisphere coverage.
        DishLimits {
            max_azimuth_rate_deg_s: 8.0,
            max_elevation_rate_deg_s: 8.0,
            min_elevation_deg: 0.0,
            max_elevation_deg: 90.0,
        }
    }
}

/// Errors returned by a [`DishBackend`].
#[derive(Debug, Clone, PartialEq)]
pub enum DishError {
    /// Commanded elevation is below the mechanical horizon.
    BelowHorizon {
        /// The commanded elevation, degrees.
        commanded_deg: f64,
        /// The backend's minimum elevation, degrees.
        min_deg: f64,
    },
    /// Commanded elevation is above the mechanical ceiling.
    AboveCeiling {
        /// The commanded elevation, degrees.
        commanded_deg: f64,
        /// The backend's maximum elevation, degrees.
        max_deg: f64,
    },
    /// The commanded angle was not a finite number.
    NotFinite,
    /// A backend-specific hardware fault.
    Fault(&'static str),
}

impl fmt::Display for DishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DishError::BelowHorizon {
                commanded_deg,
                min_deg,
            } => write!(
                f,
                "commanded elevation {commanded_deg:.3} deg is below the horizon limit {min_deg:.3} deg"
            ),
            DishError::AboveCeiling {
                commanded_deg,
                max_deg,
            } => write!(
                f,
                "commanded elevation {commanded_deg:.3} deg is above the ceiling limit {max_deg:.3} deg"
            ),
            DishError::NotFinite => write!(f, "commanded angle is not finite"),
            DishError::Fault(msg) => write!(f, "dish fault: {msg}"),
        }
    }
}

impl std::error::Error for DishError {}

/// A hardware backend that accepts pointing commands and reports orientation.
///
/// Implementors must uphold the guarantees documented in the [module-level
/// interface contract](self#interface-contract). The tracking loop is generic
/// over this trait, so the same controller drives the [`SimulatedDish`] today
/// and a real positioner in the future.
pub trait DishBackend {
    /// Command the dish to point at `cmd`, returning an error if the target is
    /// outside the backend's limits. The setpoint is retained until superseded.
    fn command(&mut self, cmd: PointingCommand) -> Result<(), DishError>;

    /// Advance the actuator by `dt` (simulated time). Real, autonomously moving
    /// hardware may ignore `dt` and use this as a servicing hook.
    fn tick(&mut self, dt: Duration);

    /// Report the current orientation and slew status.
    fn state(&self) -> DishState;

    /// Report the backend's mechanical envelope and rate limits.
    fn limits(&self) -> DishLimits;
}

/// A deterministic, software-only dish positioner used for simulation and
/// testing. It models rate-limited azimuth/elevation slewing with shortest-path
/// azimuth motion (including wrap-around across 0/360 degrees).
#[derive(Debug, Clone)]
pub struct SimulatedDish {
    azimuth_deg: f64,
    elevation_deg: f64,
    target_azimuth_deg: f64,
    target_elevation_deg: f64,
    limits: DishLimits,
}

impl SimulatedDish {
    /// Create a dish parked at azimuth 0, elevation at the horizon limit.
    pub fn new(limits: DishLimits) -> Self {
        SimulatedDish {
            azimuth_deg: 0.0,
            elevation_deg: limits.min_elevation_deg,
            target_azimuth_deg: 0.0,
            target_elevation_deg: limits.min_elevation_deg,
            limits,
        }
    }

    /// Create a dish already oriented at the given azimuth/elevation.
    pub fn parked_at(azimuth_deg: f64, elevation_deg: f64, limits: DishLimits) -> Self {
        let az = wrap_360(azimuth_deg);
        let el = elevation_deg.clamp(limits.min_elevation_deg, limits.max_elevation_deg);
        SimulatedDish {
            azimuth_deg: az,
            elevation_deg: el,
            target_azimuth_deg: az,
            target_elevation_deg: el,
            limits,
        }
    }
}

impl DishBackend for SimulatedDish {
    fn command(&mut self, cmd: PointingCommand) -> Result<(), DishError> {
        if !cmd.azimuth_deg.is_finite() || !cmd.elevation_deg.is_finite() {
            return Err(DishError::NotFinite);
        }
        if cmd.elevation_deg < self.limits.min_elevation_deg {
            return Err(DishError::BelowHorizon {
                commanded_deg: cmd.elevation_deg,
                min_deg: self.limits.min_elevation_deg,
            });
        }
        if cmd.elevation_deg > self.limits.max_elevation_deg {
            return Err(DishError::AboveCeiling {
                commanded_deg: cmd.elevation_deg,
                max_deg: self.limits.max_elevation_deg,
            });
        }
        self.target_azimuth_deg = wrap_360(cmd.azimuth_deg);
        self.target_elevation_deg = cmd.elevation_deg;
        Ok(())
    }

    fn tick(&mut self, dt: Duration) {
        let secs = dt.as_secs_f64();
        let max_az_step = self.limits.max_azimuth_rate_deg_s * secs;
        let max_el_step = self.limits.max_elevation_rate_deg_s * secs;

        // Azimuth: move along the shortest signed arc toward the target.
        let daz = shortest_arc(self.azimuth_deg, self.target_azimuth_deg);
        let az_step = daz.clamp(-max_az_step, max_az_step);
        self.azimuth_deg = wrap_360(self.azimuth_deg + az_step);

        // Elevation: linear motion within limits.
        let del = self.target_elevation_deg - self.elevation_deg;
        let el_step = del.clamp(-max_el_step, max_el_step);
        self.elevation_deg += el_step;
    }

    fn state(&self) -> DishState {
        let az_remaining = shortest_arc(self.azimuth_deg, self.target_azimuth_deg).abs();
        let el_remaining = (self.target_elevation_deg - self.elevation_deg).abs();
        DishState {
            azimuth_deg: self.azimuth_deg,
            elevation_deg: self.elevation_deg,
            slewing: az_remaining > 1e-9 || el_remaining > 1e-9,
        }
    }

    fn limits(&self) -> DishLimits {
        self.limits
    }
}

/// Normalize an angle to `[0, 360)` degrees.
pub fn wrap_360(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

/// Signed shortest arc from `from` to `to`, in `(-180, 180]` degrees.
pub fn shortest_arc(from: f64, to: f64) -> f64 {
    let mut d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> DishLimits {
        DishLimits {
            max_azimuth_rate_deg_s: 10.0,
            max_elevation_rate_deg_s: 10.0,
            min_elevation_deg: 0.0,
            max_elevation_deg: 90.0,
        }
    }

    #[test]
    fn slews_toward_target_and_settles() {
        let mut dish = SimulatedDish::new(caps());
        dish.command(PointingCommand::new(90.0, 45.0)).unwrap();
        // Az 0->90 deg at 10 deg/s needs 9 s; step 12 s of 100 ms ticks.
        for _ in 0..120 {
            dish.tick(Duration::from_millis(100));
        }
        let s = dish.state();
        assert!((s.azimuth_deg - 90.0).abs() < 1e-6, "az {}", s.azimuth_deg);
        assert!(
            (s.elevation_deg - 45.0).abs() < 1e-6,
            "el {}",
            s.elevation_deg
        );
        assert!(!s.slewing);
    }

    #[test]
    fn respects_rate_limit() {
        let mut dish = SimulatedDish::new(caps());
        dish.command(PointingCommand::new(0.0, 90.0)).unwrap();
        dish.tick(Duration::from_secs(1)); // 10 deg/s -> 10 deg in 1 s
        let s = dish.state();
        assert!(
            (s.elevation_deg - 10.0).abs() < 1e-9,
            "el {}",
            s.elevation_deg
        );
        assert!(s.slewing);
    }

    #[test]
    fn azimuth_takes_shortest_path_across_wrap() {
        let mut dish = SimulatedDish::parked_at(350.0, 10.0, caps());
        dish.command(PointingCommand::new(10.0, 10.0)).unwrap();
        // Shortest arc from 350 -> 10 is +20 deg, not -340.
        dish.tick(Duration::from_secs(1)); // 10 deg step
        let s = dish.state();
        assert!((s.azimuth_deg - 0.0).abs() < 1e-9 || (s.azimuth_deg - 360.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_below_horizon() {
        let mut dish = SimulatedDish::new(caps());
        let err = dish.command(PointingCommand::new(0.0, -5.0)).unwrap_err();
        assert!(matches!(err, DishError::BelowHorizon { .. }));
    }

    #[test]
    fn rejects_non_finite() {
        let mut dish = SimulatedDish::new(caps());
        assert_eq!(
            dish.command(PointingCommand::new(f64::NAN, 10.0)),
            Err(DishError::NotFinite)
        );
    }

    #[test]
    fn shortest_arc_signs() {
        assert!((shortest_arc(350.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((shortest_arc(10.0, 350.0) + 20.0).abs() < 1e-9);
        assert!((shortest_arc(0.0, 180.0) - 180.0).abs() < 1e-9);
    }
}
