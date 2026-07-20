// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ground-station visibility: look angles (azimuth/elevation) and AOS/LOS
//! acquisition/loss-of-signal window calculation against a propagated track.

use crate::constants::*;
use crate::error::Result;
use crate::propagator::{Propagator, StateVector};

/// A ground station location on the Earth's surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundStation {
    /// Geodetic latitude, degrees (north positive).
    pub latitude_deg: f64,
    /// Longitude, degrees (east positive).
    pub longitude_deg: f64,
    /// Height above the ellipsoid, kilometers.
    pub altitude_km: f64,
    /// Minimum elevation mask, degrees. Passes below this are not visible.
    pub min_elevation_deg: f64,
}

impl GroundStation {
    /// Geocentric (ECEF) position of the station in kilometers.
    pub fn ecef_km(&self) -> [f64; 3] {
        let lat = self.latitude_deg * DEG2RAD;
        let lon = self.longitude_deg * DEG2RAD;
        let f = EARTH_FLATTENING;
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        let n = EARTH_RADIUS_KM / (1.0 - f * (2.0 - f) * sin_lat * sin_lat).sqrt();
        let h = self.altitude_km;
        let x = (n + h) * cos_lat * lon.cos();
        let y = (n + h) * cos_lat * lon.sin();
        let z = (n * (1.0 - f * (2.0 - f)) + h) * sin_lat;
        [x, y, z]
    }
}

/// Topocentric look angles from a ground station toward a satellite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookAngles {
    /// Azimuth, degrees from true north (0..360).
    pub azimuth_deg: f64,
    /// Elevation, degrees above the local horizon (negative below horizon).
    pub elevation_deg: f64,
    /// Slant range, kilometers from station to satellite.
    pub range_km: f64,
}

impl LookAngles {
    /// Whether the satellite is above the station's elevation mask.
    pub fn is_visible(&self, min_elevation_deg: f64) -> bool {
        self.elevation_deg >= min_elevation_deg
    }
}

/// Compute topocentric look angles from `station` to a satellite at `state`.
///
/// `state` is a TEME position; `gmst_rad` is the Greenwich Mean Sidereal Time
/// (radians) at the observation instant, used to rotate TEME -> ECEF -> SEZ.
pub fn look_angles(
    station: &GroundStation,
    state: &StateVector,
    gmst_rad: f64,
) -> LookAngles {
    let rs = station.ecef_km();
    let r_sat = state.position_km;

    // TEME -> ECEF rotation about z by gmst.
    let (sin_g, cos_g) = gmst_rad.sin_cos();
    let r_sat_ecef = [
        cos_g * r_sat[0] + sin_g * r_sat[1],
        -sin_g * r_sat[0] + cos_g * r_sat[1],
        r_sat[2],
    ];

    // SEZ (south-east-zenith) vector in the station's local frame.
    let rho = [
        r_sat_ecef[0] - rs[0],
        r_sat_ecef[1] - rs[1],
        r_sat_ecef[2] - rs[2],
    ];
    let lat = station.latitude_deg * DEG2RAD;
    let lon = station.longitude_deg * DEG2RAD;
    let (sin_lat, cos_lat) = lat.sin_cos();
    let (sin_lon, cos_lon) = lon.sin_cos();

    let r_sez = [
        sin_lat * cos_lon * rho[0] + sin_lat * sin_lon * rho[1] - cos_lat * rho[2],
        -sin_lon * rho[0] + cos_lon * rho[1],
        cos_lat * cos_lon * rho[0] + cos_lat * sin_lon * rho[1] + sin_lat * rho[2],
    ];

    let range = (r_sez[0].powi(2) + r_sez[1].powi(2) + r_sez[2].powi(2)).sqrt();
    let elev = (r_sez[2] / range).asin();

    // Azimuth measured clockwise from south toward east.
    let mut az = r_sez[1].atan2(-r_sez[0]);
    if az < 0.0 {
        az += 2.0 * std::f64::consts::PI;
    }

    LookAngles {
        azimuth_deg: az * RAD2DEG,
        elevation_deg: elev * RAD2DEG,
        range_km: range,
    }
}

/// An acquisition/loss-of-signal visibility window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisibilityWindow {
    /// Minutes after the TLE epoch at acquisition of signal (AOS).
    pub aos_tsince_min: f64,
    /// Minutes after the TLE epoch at loss of signal (LOS).
    pub los_tsince_min: f64,
    /// Maximum elevation reached during the pass, degrees.
    pub max_elevation_deg: f64,
    /// Look angles at the moment of maximum elevation.
    pub max_elevation_look: LookAngles,
}

/// Trace a satellite track and return all visibility windows above the
/// station's elevation mask.
///
/// `start_min`/`end_min` bound the search window in minutes after epoch,
/// sampled at `step_min` resolution. The window endpoints are refined to the
/// elevation-mask crossing by linear interpolation between samples.
pub fn visibility_windows(
    sat: &Propagator,
    station: &GroundStation,
    start_min: f64,
    end_min: f64,
    step_min: f64,
) -> Result<Vec<VisibilityWindow>> {
    let mut windows: Vec<VisibilityWindow> = Vec::new();
    let mask = station.min_elevation_deg;

    let mut t = start_min;
    let mut prev_t = start_min;
    let mut prev_visible = false;
    let mut prev_elev = -90.0;
    let mut open_aos: Option<f64> = None;

    while t <= end_min + 1e-9 {
        let state = sat.propagate(t)?;
        let gmst = gmst_at(t);
        let la = look_angles(station, &state, gmst);
        let visible = la.elevation_deg >= mask;

        if visible && !prev_visible {
            let aos = refine_crossing(sat, station, prev_t, t, mask, prev_elev, la.elevation_deg);
            open_aos = Some(aos);
        }
        if !visible && prev_visible {
            if let Some(aos) = open_aos.take() {
                let los = refine_crossing(sat, station, prev_t, t, mask, prev_elev, la.elevation_deg);
                if let Some(w) = build_window(sat, station, aos, los) {
                    windows.push(w);
                }
            }
        }
        prev_visible = visible;
        prev_t = t;
        prev_elev = la.elevation_deg;
        t += step_min;
    }
    // Any pass still open at end_min is intentionally dropped (incomplete).
    Ok(windows)
}

/// Build a finished [`VisibilityWindow`] by locating the maximum elevation
/// between `aos` and `los`.
fn build_window(
    sat: &Propagator,
    station: &GroundStation,
    aos: f64,
    los: f64,
) -> Option<VisibilityWindow> {
    let mut max_elev = -90.0;
    let mut best_t = aos;
    let span = los - aos;
    let n = 200_usize;
    for k in 0..=n {
        let tt = aos + span * (k as f64) / (n as f64);
        let st = sat.propagate(tt).ok()?;
        let la = look_angles(station, &st, gmst_at(tt));
        if la.elevation_deg > max_elev {
            max_elev = la.elevation_deg;
            best_t = tt;
        }
    }
    let best_state = sat.propagate(best_t).ok()?;
    let best_look = look_angles(station, &best_state, gmst_at(best_t));
    Some(VisibilityWindow {
        aos_tsince_min: aos,
        los_tsince_min: los,
        max_elevation_deg: max_elev,
        max_elevation_look: best_look,
    })
}

fn refine_crossing(
    sat: &Propagator,
    station: &GroundStation,
    t0: f64,
    t1: f64,
    mask: f64,
    e0: f64,
    e1: f64,
) -> f64 {
    // Linear interpolation of elevation between (t0,e0) and (t1,e1) at `mask`.
    if (e1 - e0).abs() < 1e-12 {
        return t1;
    }
    let frac = (mask - e0) / (e1 - e0);
    let guess = t0 + frac * (t1 - t0);
    // One Newton step on elevation using central difference.
    let h = 0.5;
    let st_m = sat.propagate(guess - h).ok();
    let st_p = sat.propagate(guess + h).ok();
    if let (Some(a), Some(b)) = (st_m, st_p) {
        let em = look_angles(station, &a, gmst_at(guess - h)).elevation_deg;
        let ep = look_angles(station, &b, gmst_at(guess + h)).elevation_deg;
        let deriv = (ep - em) / (2.0 * h);
        if deriv.abs() > 1e-9 {
            return (guess - (e1 - mask) / deriv).clamp(t0, t1);
        }
    }
    guess
}

/// Approximate Greenwich Mean Sidereal Time (radians) at `tsince_min` after
/// the J2000 reference. A simplified linear model is adequate for simulation.
pub fn gmst_at(tsince_min: f64) -> f64 {
    // GMST at J2000 = 280.46 deg; Earth rotates 360.9856 deg/day.
    let days = tsince_min / MIN_PER_DAY;
    let deg = 280.46 + 360.985647366 * (days + 0.0);
    (deg * DEG2RAD).rem_euclid(2.0 * std::f64::consts::PI)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tle::Tle;

    const ISS: &str = "ISS (ZARYA)\n\
1 25544U 98067A   24015.50000000  .00016717  00000-0  10270-3 0  9004\n\
2 25544  51.6400 208.9163 0007652 360.0000 130.3994 15.49815308 90008";

    #[test]
    fn station_ecef_reasonable() {
        let gs = GroundStation {
            latitude_deg: 0.0,
            longitude_deg: 0.0,
            altitude_km: 0.0,
            min_elevation_deg: 0.0,
        };
        let p = gs.ecef_km();
        let r = (p[0].powi(2) + p[1].powi(2) + p[2].powi(2)).sqrt();
        assert!((r - EARTH_RADIUS_KM).abs() < 1.0);
    }

    #[test]
    fn iss_has_passes_over_midlat_station() {
        let tle = Tle::parse(ISS).unwrap();
        let sat = Propagator::from_tle(&tle).unwrap();
        let gs = GroundStation {
            latitude_deg: 35.0,
            longitude_deg: 139.0,
            altitude_km: 0.0,
            min_elevation_deg: 5.0,
        };
        let windows = visibility_windows(&sat, &gs, 0.0, 24.0 * 60.0, 1.0).unwrap();
        // ISS passes several times per day; at least one window expected.
        assert!(!windows.is_empty(), "expected at least one visibility window");
        for w in &windows {
            assert!(w.max_elevation_deg >= gs.min_elevation_deg);
            assert!(w.los_tsince_min > w.aos_tsince_min);
        }
    }
}
