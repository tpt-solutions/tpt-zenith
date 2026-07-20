// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sub-degree tracking accuracy tests against simulated satellite passes.
//!
//! These drive the deterministic tracking loop with a [`SimulatedDish`] over
//! real ISS pass geometry computed by the orbital-mechanics engine, and assert
//! the antenna follows the satellite to within a fraction of a degree.

use antenna_control::hal::{DishLimits, SimulatedDish};
use antenna_control::tracking::{TrackConfig, Tracker};
use orbital_mechanics::tle::Tle;
use orbital_mechanics::visibility::{visibility_windows, GroundStation, VisibilityWindow};
use orbital_mechanics::Propagator;

const ISS: &str = "ISS (ZARYA)\n\
1 25544U 98067A   24015.50000000  .00016717  00000-0  10270-3 0  9004\n\
2 25544  51.6400 208.9163 0007652 360.0000 130.3994 15.49815308 90008";

fn station() -> GroundStation {
    GroundStation {
        latitude_deg: 35.68,
        longitude_deg: 139.69,
        altitude_km: 0.04,
        min_elevation_deg: 10.0,
    }
}

fn iss() -> Propagator {
    let tle = Tle::parse(ISS).unwrap();
    Propagator::from_tle(&tle).unwrap()
}

/// Choose a pass whose maximum elevation lies in `[lo, hi]`, avoiding the
/// near-zenith "keyhole" passes where azimuth rate diverges.
fn pick_pass(windows: &[VisibilityWindow], lo: f64, hi: f64) -> VisibilityWindow {
    windows
        .iter()
        .filter(|w| w.max_elevation_deg >= lo && w.max_elevation_deg <= hi)
        .copied()
        .next()
        .expect("a pass in the requested elevation band")
}

/// A realistic 1 Hz control loop tracking a moderate pass, with no feedforward,
/// must still keep the antenna pointed to well under a degree.
#[test]
fn tracks_moderate_pass_to_sub_degree() {
    let sat = iss();
    let gs = station();
    let windows = visibility_windows(&sat, &gs, 0.0, 24.0 * 60.0, 0.5).unwrap();
    let pass = pick_pass(&windows, 20.0, 55.0);

    let mut dish = SimulatedDish::new(DishLimits::default());
    let config = TrackConfig {
        tick_secs: 1.0,
        ..TrackConfig::default()
    };
    let tracker = Tracker::new(&sat, gs);
    let report = tracker
        .track(&mut dish, pass.aos_tsince_min, pass.los_tsince_min, config)
        .unwrap();

    eprintln!(
        "no-lead: max_el={:.1} deg  n={}  max_err={:.4} deg  rms_err={:.4} deg",
        pass.max_elevation_deg,
        report.samples.len(),
        report.max_error_deg,
        report.rms_error_deg,
    );

    assert!(report.acquired, "dish failed to acquire before tracking");
    // The loop measures a genuine, non-trivial error...
    assert!(
        report.max_error_deg > 0.0,
        "expected a measurable tracking error"
    );
    // ...that is nonetheless comfortably sub-degree.
    assert!(
        report.max_error_deg < 1.0,
        "max pointing error {:.4} deg exceeded 1 deg",
        report.max_error_deg
    );
    assert!(
        report.rms_error_deg < 0.5,
        "rms pointing error {:.4} deg exceeded 0.5 deg",
        report.rms_error_deg
    );
}

/// Velocity feedforward should substantially reduce the tracking error versus
/// an identical run without it.
#[test]
fn feedforward_reduces_error() {
    let sat = iss();
    let gs = station();
    let windows = visibility_windows(&sat, &gs, 0.0, 24.0 * 60.0, 0.5).unwrap();
    let pass = pick_pass(&windows, 20.0, 55.0);

    let config = TrackConfig {
        tick_secs: 1.0,
        ..TrackConfig::default()
    };

    let mut plain = SimulatedDish::new(DishLimits::default());
    let no_lead = Tracker::new(&sat, gs)
        .track(&mut plain, pass.aos_tsince_min, pass.los_tsince_min, config)
        .unwrap();

    let mut led = SimulatedDish::new(DishLimits::default());
    let with_lead = Tracker::new(&sat, gs)
        .with_lead(config.tick_secs / 60.0)
        .track(&mut led, pass.aos_tsince_min, pass.los_tsince_min, config)
        .unwrap();

    eprintln!(
        "feedforward: no_lead_max={:.4} deg  with_lead_max={:.4} deg",
        no_lead.max_error_deg, with_lead.max_error_deg,
    );

    assert!(
        with_lead.max_error_deg < no_lead.max_error_deg,
        "feedforward did not reduce error: {:.4} vs {:.4}",
        with_lead.max_error_deg,
        no_lead.max_error_deg
    );
    assert!(with_lead.max_error_deg < 0.1);
}

/// Even a high-elevation (near-keyhole) pass, which stresses azimuth slew rate,
/// stays within a degree when the loop runs fast with feedforward.
#[test]
fn high_pass_stays_bounded() {
    let sat = iss();
    let gs = station();
    let windows = visibility_windows(&sat, &gs, 0.0, 24.0 * 60.0, 0.5).unwrap();
    // The highest-elevation pass available.
    let pass = windows
        .iter()
        .copied()
        .max_by(|a, b| {
            a.max_elevation_deg
                .partial_cmp(&b.max_elevation_deg)
                .unwrap()
        })
        .expect("at least one pass");

    let limits = DishLimits {
        max_azimuth_rate_deg_s: 20.0,
        max_elevation_rate_deg_s: 20.0,
        ..DishLimits::default()
    };
    let mut dish = SimulatedDish::new(limits);
    let config = TrackConfig {
        tick_secs: 0.2,
        ..TrackConfig::default()
    };
    let report = Tracker::new(&sat, gs)
        .with_lead(config.tick_secs / 60.0)
        .track(&mut dish, pass.aos_tsince_min, pass.los_tsince_min, config)
        .unwrap();

    eprintln!(
        "high pass: max_el={:.1} deg  max_err={:.4} deg  rms_err={:.4} deg",
        pass.max_elevation_deg, report.max_error_deg, report.rms_error_deg,
    );
    assert!(
        report.max_error_deg < 1.0,
        "high pass max error {:.4} deg exceeded 1 deg",
        report.max_error_deg
    );
}
