// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Orbital Mechanics Engine (Phase 1, simulation-only).
//!
//! Provides satellite position propagation (SGP4/SDP4), ground-station
//! visibility window calculation, and handoff optimization. This crate is
//! software-only simulation; no live data feeds are required.

pub mod constellation;
pub mod handoff;
pub mod propagator;
pub mod visibility;

/// Re-exports of the most commonly used items.
pub use crate::propagator::{StateVector, Tle};
