// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Antenna Control System (Phase 3, simulation-first).
//!
//! Provides a deterministic tracking loop that converts orbital-mechanics
//! pointing outputs into antenna commands, plus a hardware abstraction layer
//! with a simulated dish backend. The interface contract is designed so a
//! real microcontroller backend can replace the simulation later.

pub mod hal;
pub mod tracking;

pub use crate::hal::{
    DishBackend, DishError, DishLimits, DishState, PointingCommand, SimulatedDish,
};
pub use crate::tracking::{TrackConfig, TrackSample, Tracker, TrackingReport};
