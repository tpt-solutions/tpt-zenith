// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Error types for the rf-pipeline crate.

use std::fmt;

/// Errors that can occur while building or parsing a CCSDS Space Packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// A header field's value doesn't fit its bit width.
    FieldOutOfRange { field: &'static str, value: i64 },
    /// A CCSDS packet version other than 1 (encoded as `0b000`) was seen.
    UnsupportedVersion(u16),
    /// Fewer than 6 bytes were supplied, too short for a primary header.
    Truncated,
    /// The header's declared data length didn't match the actual payload.
    LengthMismatch { declared: usize, actual: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::FieldOutOfRange { field, value } => {
                write!(f, "field '{field}' value {value} out of range")
            }
            FrameError::UnsupportedVersion(v) => {
                write!(f, "unsupported CCSDS packet version {v}")
            }
            FrameError::Truncated => write!(f, "fewer than 6 bytes: not a valid primary header"),
            FrameError::LengthMismatch { declared, actual } => {
                write!(f, "header declared {declared} data bytes, found {actual}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Convenience `Result` alias for this crate.
pub type Result<T> = std::result::Result<T, FrameError>;
