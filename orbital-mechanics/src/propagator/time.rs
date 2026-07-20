// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pure time/sidereal-time helpers shared by `init` and `deep_space`.

use super::TWOPId;

/// Greenwich Mean Sidereal Time (radians) at a Julian date (UT1 days).
pub(super) fn gstime(jdut1: f64) -> f64 {
    let deg2rad = std::f64::consts::PI / 180.0;
    let tut1 = (jdut1 - 2451545.0) / 36525.0;
    let mut temp =
        -6.2e-6 * tut1 * tut1 * tut1 + 0.093104 * tut1 * tut1 + (876600.0 * 3600.0 + 8640184.812866) * tut1
            + 67310.54841;
    temp = (temp * deg2rad / 240.0) % TWOPId;
    if temp < 0.0 {
        temp += TWOPId;
    }
    temp
}

/// Convert a TLE epoch (MJD) into "days from Jan 0, 1950 0h" used by initl/gstime.
pub(super) fn epoch_days_1950(mjd: f64) -> f64 {
    // MJD 33281.0 == 1950-01-01T00:00; Jan 0 1950 == MJD 33280.0.
    mjd - 33280.0
}
