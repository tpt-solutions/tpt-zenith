// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Export a DTN contact plan (JSON) from Phase 1 visibility windows.
//!
//! This is the bridge between the orbital-mechanics engine (Phase 1) and the
//! Go orbital routing protocol (Phase 2). It propagates one or more satellites
//! over a set of ground stations, computes AOS/LOS visibility windows, and
//! emits each window as a pair of directed contacts (uplink and downlink) that
//! the routing layer can load into its contact-graph router.
//!
//! Times are expressed in minutes after the propagation epoch; the `epoch`
//! field gives the absolute UTC instant that offset is measured from. Run with:
//!
//! ```text
//! cargo run -p orbital-mechanics --bin export_contacts > contacts.json
//! ```

use orbital_mechanics::tle::Tle;
use orbital_mechanics::visibility::{visibility_windows, GroundStation};
use orbital_mechanics::Propagator;

/// Speed of light, kilometers per second, for one-way light-time estimates.
const C_KM_S: f64 = 299_792.458;

/// Downlink/uplink capacity assumed for every contact, in bytes per second
/// (~1 Mbit/s). The routing layer uses this to schedule transmission volume.
const DATA_RATE_BYTES_PER_SEC: f64 = 125_000.0;

struct Station {
    node: &'static str,
    gs: GroundStation,
}

fn main() {
    // ISS (ZARYA), epoch 24015.5 == 2024-01-15 12:00:00 UTC.
    const ISS: &str = "ISS (ZARYA)\n\
1 25544U 98067A   24015.50000000  .00016717  00000-0  10270-3 0  9004\n\
2 25544  51.6400 208.9163 0007652 360.0000 130.3994 15.49815308 90008";
    let epoch = "2024-01-15T12:00:00Z";
    let sat_node = "dtn://sat-25544";

    let tle = Tle::parse(ISS).expect("parse ISS TLE");
    let sat = Propagator::from_tle(&tle).expect("init propagator");

    let stations = [
        Station {
            node: "dtn://ground-tokyo",
            gs: GroundStation {
                latitude_deg: 35.68,
                longitude_deg: 139.69,
                altitude_km: 0.04,
                min_elevation_deg: 10.0,
            },
        },
        Station {
            node: "dtn://ground-kauai",
            gs: GroundStation {
                latitude_deg: 22.09,
                longitude_deg: -159.5,
                altitude_km: 0.01,
                min_elevation_deg: 10.0,
            },
        },
    ];

    // Search a full day at 0.5-minute resolution.
    let (start_min, end_min, step_min) = (0.0, 24.0 * 60.0, 0.5);

    let mut contacts: Vec<String> = Vec::new();
    for st in &stations {
        let windows = visibility_windows(&sat, &st.gs, start_min, end_min, step_min)
            .expect("compute visibility windows");
        for w in &windows {
            let owlt_ms = w.max_elevation_look.range_km / C_KM_S * 1000.0;
            // Uplink: ground station -> satellite.
            contacts.push(contact_json(
                st.node,
                sat_node,
                w.aos_tsince_min,
                w.los_tsince_min,
                owlt_ms,
            ));
            // Downlink: satellite -> ground station.
            contacts.push(contact_json(
                sat_node,
                st.node,
                w.aos_tsince_min,
                w.los_tsince_min,
                owlt_ms,
            ));
        }
    }

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"epoch\": \"{epoch}\",\n"));
    out.push_str(&format!("  \"satellite\": \"{sat_node}\",\n"));
    out.push_str("  \"contacts\": [\n");
    for (i, c) in contacts.iter().enumerate() {
        let comma = if i + 1 < contacts.len() { "," } else { "" };
        out.push_str(&format!("    {c}{comma}\n"));
    }
    out.push_str("  ]\n");
    out.push_str("}\n");

    print!("{out}");
}

fn contact_json(from: &str, to: &str, start_min: f64, end_min: f64, owlt_ms: f64) -> String {
    format!(
        "{{\"from\": \"{from}\", \"to\": \"{to}\", \"start_min\": {start_min:.4}, \
\"end_min\": {end_min:.4}, \"data_rate_bytes_per_sec\": {rate:.1}, \
\"owlt_ms\": {owlt_ms:.3}, \"confidence\": 1.0}}",
        rate = DATA_RATE_BYTES_PER_SEC,
    )
}
