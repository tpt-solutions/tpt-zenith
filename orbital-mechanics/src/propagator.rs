// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! SGP4 / SDP4 orbital propagator.
//!
//! A faithful port of the Vallado et al. "Revisiting Spacetrack Report #3"
//! (AIAA 2006-6753) reference implementation. Outputs position and velocity in
//! the True Equator Mean Equinox (TEME) frame, in kilometers and kilometers per
//! second, using the WGS72 gravity constants. Validated against the official
//! `tcppver.out` verification vectors (see `tests/verification.rs`).
//!
//! Split across submodules by concern:
//! - [`time`]: pure sidereal-time helpers (`gstime`, `epoch_days_1950`).
//! - [`init`]: `Propagator::from_tle` (`sgp4init`).
//! - [`propagate`]: `Propagator::propagate` (`sgp4`).
//! - [`deep_space`]: the resonance cluster (`dsinit`, `dpper`, `dspace`).

mod deep_space;
mod init;
mod propagate;
mod time;

use crate::constants::J3OJ2;

/// Shared `xlcof`/`aycof` formula, evaluated once by `init` against the
/// near-earth inclination (`sinio`/`cosio`) and again by `propagate` against
/// the deep-space perturbed inclination (`sinip`/`cosip`) after `dpper`.
fn xlcof_aycof(sini: f64, cosi: f64) -> (f64, f64) {
    let xlcof = if (cosi + 1.0).abs() > 1.5e-12 {
        -0.25 * J3OJ2 * sini * (3.0 + 5.0 * cosi) / (1.0 + cosi)
    } else {
        -0.25 * J3OJ2 * sini * (3.0 + 5.0 * cosi) / 1.5e-12
    };
    let aycof = -0.5 * J3OJ2 * sini;
    (xlcof, aycof)
}

/// Position (km) and velocity (km/s) in the TEME inertial frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateVector {
    /// Position in kilometers [x, y, z].
    pub position_km: [f64; 3],
    /// Velocity in kilometers per second [vx, vy, vz].
    pub velocity_kms: [f64; 3],
}

impl StateVector {
    /// Geocentric radius in kilometers.
    pub fn radius_km(&self) -> f64 {
        let p = self.position_km;
        (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
    }

    /// Speed in kilometers per second.
    pub fn speed_kms(&self) -> f64 {
        let v = self.velocity_kms;
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }
}

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;
const X2O3: f64 = 2.0 / 3.0;
/// Earth rotation rate for Greenwich-angle propagation, rad/min (matches the
/// `rptim` constant used by the reference `dspace` Greenwich-angle update).
const EARTH_ROTATION_RAD_PER_MIN: f64 = 4.375_269_088_011_3e-3;

/// Initialized element set for repeated SGP4/SDP4 propagation.
///
/// Created via [`Propagator::from_tle`]. Holds the full `elsetrec` state from
/// the Vallado reference implementation.
#[derive(Debug, Clone)]
pub struct Propagator {
    // near-earth constants
    isimp: i32,
    method: char,
    aycof: f64,
    con41: f64,
    cc1: f64,
    cc4: f64,
    cc5: f64,
    d2: f64,
    d3: f64,
    d4: f64,
    delmo: f64,
    eta: f64,
    argpdot: f64,
    omgcof: f64,
    sinmao: f64,
    t2cof: f64,
    t3cof: f64,
    t4cof: f64,
    t5cof: f64,
    x1mth2: f64,
    x7thm1: f64,
    mdot: f64,
    nodecf: f64,
    nodedot: f64,
    xlcof: f64,
    xmcof: f64,
    no_kozai: f64,
    // epoch elements
    bstar: f64,
    ecco: f64,
    argpo: f64,
    inclo: f64,
    mo: f64,
    nodeo: f64,
    // deep space
    irez: i32,
    d2201: f64,
    d2211: f64,
    d3210: f64,
    d3222: f64,
    d4410: f64,
    d4422: f64,
    d5220: f64,
    d5232: f64,
    d5421: f64,
    d5433: f64,
    dedt: f64,
    del1: f64,
    del2: f64,
    del3: f64,
    didt: f64,
    dmdt: f64,
    dnodt: f64,
    domdt: f64,
    e3: f64,
    ee2: f64,
    peo: f64,
    pgho: f64,
    pho: f64,
    pinco: f64,
    plo: f64,
    se2: f64,
    se3: f64,
    sgh2: f64,
    sgh3: f64,
    sgh4: f64,
    sh2: f64,
    sh3: f64,
    si2: f64,
    si3: f64,
    sl2: f64,
    sl3: f64,
    sl4: f64,
    xgh2: f64,
    xgh3: f64,
    xgh4: f64,
    xh2: f64,
    xh3: f64,
    xi2: f64,
    xi3: f64,
    xl2: f64,
    xl3: f64,
    xl4: f64,
    zmol: f64,
    zmos: f64,
    atime: f64,
    xli: f64,
    xni: f64,
    gsto: f64,
    xfact: f64,
    xlamo: f64,
}

impl Propagator {
    /// Greenwich Mean Sidereal Time (radians, wrapped to `[0, 2*pi)`) at
    /// `tsince_min` minutes after this propagator's TLE epoch.
    ///
    /// Used to rotate a TEME state vector (as returned by [`propagate`]) into
    /// ECEF, e.g. for ground-station look-angle calculations.
    ///
    /// [`propagate`]: Propagator::propagate
    pub fn gmst_rad(&self, tsince_min: f64) -> f64 {
        (self.gsto + tsince_min * EARTH_ROTATION_RAD_PER_MIN).rem_euclid(TWO_PI)
    }
}
