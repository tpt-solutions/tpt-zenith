// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Physical and mathematical constants used across the propagator.

/// Earth's equatorial radius (km). WGS72 value used by SGP4.
pub const EARTH_RADIUS_KM: f64 = 6378.137;
/// Earth gravitational parameter mu (km^3 / s^2), WGS72.
pub const MU_EARTH: f64 = 398600.79964;
/// Kepler's constant for SGP4 in the earth-radius unit convention used by the
/// Vallado et al. reference implementation. With `mu` in km^3/s^2 and the
/// Earth's equatorial radius in km, this evaluates to
/// `60 * sqrt(mu) / radius_km^1.5` so that the resulting semimajor axis is
/// expressed in earth radii (and is later multiplied by `EARTH_RADIUS_KM` to
/// obtain kilometers). Equal to `7.43668811206102e-2`.
pub const XKE: f64 = 7.43668811206102e-02;
/// Earth rotation rate (rad/s), WGS72.
pub const EARTH_ROTATION_RAD_S: f64 = 7.292115e-5;
/// Second zonal harmonic J2, WGS72.
pub const J2: f64 = 1.082616e-3;
/// Third zonal harmonic J3, WGS72.
pub const J3: f64 = -2.53881e-6;
/// Fourth zonal harmonic J4, WGS72.
pub const J4: f64 = -1.65597e-6;
/// J3 / J2 ratio, used by the lunar/solar periodic terms.
pub const J3OJ2: f64 = J3 / J2;
/// Second Earth flattening factor, WGS72.
pub const EARTH_FLATTENING: f64 = 1.0 / 298.26;

/// Minutes in a day.
pub const MIN_PER_DAY: f64 = 1440.0;
/// Seconds in a day.
pub const SEC_PER_DAY: f64 = 86400.0;
/// Radians per degree.
pub const DEG2RAD: f64 = std::f64::consts::PI / 180.0;
/// Degrees per radian.
pub const RAD2DEG: f64 = 180.0 / std::f64::consts::PI;
