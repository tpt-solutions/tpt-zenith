// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Handoff optimization: choose the next satellite to track as the currently
//! tracked satellite sets below the horizon.

use crate::propagator::Propagator;
use crate::visibility::{look_angles, visibility_windows, GroundStation, VisibilityWindow};

/// A scheduled handoff from one satellite to the next.
#[derive(Debug, Clone)]
pub struct Handoff {
    /// Index of the satellite being handed off from (the one setting).
    pub from_index: usize,
    /// Index of the satellite to hand off to (the one rising).
    pub to_index: usize,
    /// Minutes after epoch at which the handoff should occur.
    pub at_tsince_min: f64,
    /// Elevation (degrees) of the incoming satellite at the handoff instant.
    pub incoming_elevation_deg: f64,
}

/// Greedily build a handoff schedule across a constellation over a time window.
///
/// For each satellite, its visibility windows are computed. When one satellite
/// loses signal (LOS), the algorithm selects the highest-elevation visible
/// alternative (excluding the one setting) at that instant as the handoff
/// target. Returns the list of handoffs found.
pub fn plan_handoffs(
    satellites: &[Propagator],
    station: &GroundStation,
    start_min: f64,
    end_min: f64,
    step_min: f64,
) -> Vec<Handoff> {
    // Precompute visibility windows per satellite.
    let windows: Vec<Vec<VisibilityWindow>> = satellites
        .iter()
        .map(|s| visibility_windows(s, station, start_min, end_min, step_min).unwrap_or_default())
        .collect();

    let mut handoffs = Vec::new();

    for (from, from_windows) in windows.iter().enumerate() {
        for w in from_windows {
            let los = w.los_tsince_min;
            // Candidate: any other satellite visible (above mask) at `los`,
            // ranked by its actual elevation at that instant.
            let mut best: Option<(usize, f64)> = None;
            for (to, to_windows) in windows.iter().enumerate() {
                if to == from {
                    continue;
                }
                if !to_windows
                    .iter()
                    .any(|tw| los >= tw.aos_tsince_min && los <= tw.los_tsince_min)
                {
                    continue;
                }
                let Ok(state) = satellites[to].propagate(los) else {
                    continue;
                };
                let elev = look_angles(station, &state, satellites[to].gmst_rad(los)).elevation_deg;
                if best.map_or(true, |b| elev > b.1) {
                    best = Some((to, elev));
                }
            }
            if let Some((to, elev)) = best {
                handoffs.push(Handoff {
                    from_index: from,
                    to_index: to,
                    at_tsince_min: los,
                    incoming_elevation_deg: elev,
                });
            }
        }
    }
    handoffs.sort_by(|a, b| a.at_tsince_min.partial_cmp(&b.at_tsince_min).unwrap());
    handoffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tle::Tle;
    use crate::visibility::GroundStation;

    #[test]
    fn empty_when_no_satellites() {
        let gs = GroundStation {
            latitude_deg: 0.0,
            longitude_deg: 0.0,
            altitude_km: 0.0,
            min_elevation_deg: 5.0,
        };
        let plan = plan_handoffs(&[], &gs, 0.0, 200.0, 2.0);
        assert!(plan.is_empty());
    }
}
