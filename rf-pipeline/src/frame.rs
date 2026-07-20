// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CCSDS 133.0-B-2 Space Packet Protocol primary header: the 6-byte framing
//! unit that would sit between this crate's FEC/modem chain and the DTN
//! bundle layer (Phase 2). See `docs/ccsds-compliance.md` for what of the
//! wider CCSDS stack this does and does not cover.

use crate::error::{FrameError, Result};

/// Packet type, per CCSDS 133.0-B-2 section 4.1.3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Telemetry (spacecraft -> ground).
    Telemetry,
    /// Telecommand (ground -> spacecraft).
    Telecommand,
}

/// Sequence flags, per CCSDS 133.0-B-2 section 4.1.3.4: whether this packet
/// is a standalone user-data unit, or a segment of one split across packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceFlags {
    /// A continuation segment (neither first nor last).
    Continuation,
    /// The first segment of a split user-data unit.
    First,
    /// The last segment of a split user-data unit.
    Last,
    /// A complete, unsegmented user-data unit (the common case).
    Unsegmented,
}

impl SequenceFlags {
    fn bits(self) -> u8 {
        match self {
            SequenceFlags::Continuation => 0b00,
            SequenceFlags::First => 0b01,
            SequenceFlags::Last => 0b10,
            SequenceFlags::Unsegmented => 0b11,
        }
    }

    fn from_bits(b: u8) -> Self {
        match b & 0b11 {
            0b00 => SequenceFlags::Continuation,
            0b01 => SequenceFlags::First,
            0b10 => SequenceFlags::Last,
            _ => SequenceFlags::Unsegmented,
        }
    }
}

/// The 6-byte CCSDS Space Packet primary header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpacePacketHeader {
    /// Application Process ID identifying the source/destination process,
    /// `0..=2047` (11 bits).
    pub apid: u16,
    /// Telemetry or telecommand.
    pub packet_type: PacketType,
    /// Whether a secondary header follows this primary header.
    pub secondary_header: bool,
    /// Segmentation state of this packet.
    pub sequence_flags: SequenceFlags,
    /// Sequence count or packet name, `0..=16383` (14 bits); wraps per APID.
    pub sequence_count: u16,
    /// Length of the packet data field in octets.
    pub data_len: usize,
}

const MAX_APID: u16 = 0x7FF; // 11 bits
const MAX_SEQUENCE_COUNT: u16 = 0x3FFF; // 14 bits
const MAX_DATA_LEN: usize = 0x1_0000; // 16-bit (len - 1) field -> up to 65536 octets

impl SpacePacketHeader {
    /// Serialize to the 6-byte on-the-wire primary header.
    pub fn to_bytes(&self) -> Result<[u8; 6]> {
        if self.apid > MAX_APID {
            return Err(FrameError::FieldOutOfRange {
                field: "apid",
                value: self.apid as i64,
            });
        }
        if self.sequence_count > MAX_SEQUENCE_COUNT {
            return Err(FrameError::FieldOutOfRange {
                field: "sequence_count",
                value: self.sequence_count as i64,
            });
        }
        if self.data_len == 0 || self.data_len > MAX_DATA_LEN {
            return Err(FrameError::FieldOutOfRange {
                field: "data_len",
                value: self.data_len as i64,
            });
        }

        let version: u16 = 0; // CCSDS version 1 packets are encoded as 0b000.
        let type_bit: u16 = match self.packet_type {
            PacketType::Telemetry => 0,
            PacketType::Telecommand => 1,
        };
        let sec_hdr_bit: u16 = self.secondary_header as u16;

        let word0 =
            (version << 13) | (type_bit << 12) | (sec_hdr_bit << 11) | (self.apid & MAX_APID);
        let word1 = ((self.sequence_flags.bits() as u16) << 14)
            | (self.sequence_count & MAX_SEQUENCE_COUNT);
        let word2 = (self.data_len - 1) as u16;

        let mut out = [0u8; 6];
        out[0..2].copy_from_slice(&word0.to_be_bytes());
        out[2..4].copy_from_slice(&word1.to_be_bytes());
        out[4..6].copy_from_slice(&word2.to_be_bytes());
        Ok(out)
    }

    /// Parse a 6-byte primary header.
    pub fn from_bytes(b: &[u8; 6]) -> Result<Self> {
        let word0 = u16::from_be_bytes([b[0], b[1]]);
        let word1 = u16::from_be_bytes([b[2], b[3]]);
        let word2 = u16::from_be_bytes([b[4], b[5]]);

        let version = (word0 >> 13) & 0b111;
        if version != 0 {
            return Err(FrameError::UnsupportedVersion(version));
        }
        let packet_type = if (word0 >> 12) & 1 == 1 {
            PacketType::Telecommand
        } else {
            PacketType::Telemetry
        };
        let secondary_header = (word0 >> 11) & 1 == 1;
        let apid = word0 & MAX_APID;

        let sequence_flags = SequenceFlags::from_bits((word1 >> 14) as u8);
        let sequence_count = word1 & MAX_SEQUENCE_COUNT;
        let data_len = word2 as usize + 1;

        Ok(SpacePacketHeader {
            apid,
            packet_type,
            secondary_header,
            sequence_flags,
            sequence_count,
            data_len,
        })
    }
}

/// A CCSDS Space Packet: primary header plus its data field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacePacket {
    /// The primary header. `header.data_len` must equal `data.len()`.
    pub header: SpacePacketHeader,
    /// The packet data field (user data, or secondary header + user data).
    pub data: Vec<u8>,
}

impl SpacePacket {
    /// Build a packet, deriving `header.data_len` from `data`.
    pub fn new(mut header: SpacePacketHeader, data: Vec<u8>) -> Result<Self> {
        header.data_len = data.len();
        header.to_bytes()?; // validate field ranges eagerly
        Ok(SpacePacket { header, data })
    }

    /// Serialize the full packet (header followed by the data field).
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = self.header.to_bytes()?.to_vec();
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    /// Parse a full packet from its on-the-wire bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 6 {
            return Err(FrameError::Truncated);
        }
        let mut header_bytes = [0u8; 6];
        header_bytes.copy_from_slice(&bytes[0..6]);
        let header = SpacePacketHeader::from_bytes(&header_bytes)?;
        let data = &bytes[6..];
        if data.len() != header.data_len {
            return Err(FrameError::LengthMismatch {
                declared: header.data_len,
                actual: data.len(),
            });
        }
        Ok(SpacePacket {
            header,
            data: data.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header(data_len: usize) -> SpacePacketHeader {
        SpacePacketHeader {
            apid: 42,
            packet_type: PacketType::Telemetry,
            secondary_header: false,
            sequence_flags: SequenceFlags::Unsegmented,
            sequence_count: 100,
            data_len,
        }
    }

    #[test]
    fn header_round_trips() {
        let h = sample_header(16);
        let bytes = h.to_bytes().unwrap();
        assert_eq!(bytes.len(), 6);
        let parsed = SpacePacketHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, h);
    }

    #[test]
    fn packet_round_trips() {
        let data = vec![1, 2, 3, 4, 5];
        let packet = SpacePacket::new(sample_header(0), data.clone()).unwrap();
        let bytes = packet.to_bytes().unwrap();
        assert_eq!(bytes.len(), 6 + data.len());
        let parsed = SpacePacket::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.data, data);
        assert_eq!(parsed.header.data_len, data.len());
    }

    #[test]
    fn rejects_apid_out_of_range() {
        let mut h = sample_header(1);
        h.apid = MAX_APID + 1;
        assert!(h.to_bytes().is_err());
    }

    #[test]
    fn rejects_truncated_bytes() {
        assert!(SpacePacket::from_bytes(&[0, 1, 2]).is_err());
    }

    #[test]
    fn rejects_length_mismatch() {
        let mut bytes = SpacePacket::new(sample_header(0), vec![1, 2, 3])
            .unwrap()
            .to_bytes()
            .unwrap();
        bytes.pop(); // now declares 3 bytes of data but only carries 2
        assert!(SpacePacket::from_bytes(&bytes).is_err());
    }
}
