// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Physical and mathematical constants used across the propagator.

/// Earth's equatorial radius (km). WGS72 value used by SGP4.
pub const EARTH_RADIUS_KM: f64 = 6378.137;
/// Earth gravitational parameter mu (km^3 / s^2), WGS72.
pub const MU_EARTH: f64 = 398600.79964;
/// Earth rotation rate (rad/s), WGS72.
pub const EARTH_ROTATION_RAD_S: f64 = 7.292115e-5;
/// Second zonal harmonic J2, WGS72.
pub const J2: f64 = 1.082616e-3;
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
