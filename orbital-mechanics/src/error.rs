// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Error types for the orbital-mechanics crate.

use std::fmt;

/// Errors that can occur while parsing TLE data or propagating orbits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrbitError {
    /// The TLE string did not have the expected number of lines.
    InvalidLineCount(usize),
    /// A line was not the expected length (expected 69 characters).
    InvalidLineLength(usize),
    /// A checksum did not match (computed, expected).
    ChecksumMismatch(u8, u8),
    /// A numeric field could not be parsed.
    ParseField { field: &'static str, value: String },
    /// The epoch could not be interpreted.
    InvalidEpoch(String),
    /// Propagation requested at an unsupported time (e.g. before epoch).
    TimeOutOfRange(String),
}

impl fmt::Display for OrbitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrbitError::InvalidLineCount(n) => {
                write!(f, "TLE must have 2 or 3 lines, found {n}")
            }
            OrbitError::InvalidLineLength(n) => {
                write!(f, "TLE line must be 69 chars, found {n}")
            }
            OrbitError::ChecksumMismatch(c, e) => {
                write!(f, "TLE checksum mismatch: computed {c}, expected {e}")
            }
            OrbitError::ParseField { field, value } => {
                write!(f, "failed to parse TLE field '{field}': '{value}'")
            }
            OrbitError::InvalidEpoch(s) => write!(f, "invalid TLE epoch: {s}"),
            OrbitError::TimeOutOfRange(s) => write!(f, "time out of range: {s}"),
        }
    }
}

impl std::error::Error for OrbitError {}

/// Convenience `Result` alias for this crate.
pub type Result<T> = std::result::Result<T, OrbitError>;
