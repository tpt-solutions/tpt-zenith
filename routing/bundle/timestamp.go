// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package bundle

import (
	"hash/crc32"
	"time"
)

// DTN time is the number of milliseconds elapsed since the DTN epoch,
// 2000-01-01T00:00:00Z, excluding leap seconds (RFC 9171 section 4.1.6).
var dtnEpoch = time.Date(2000, time.January, 1, 0, 0, 0, 0, time.UTC)

// DTNTime converts a wall-clock instant to DTN time (milliseconds since the
// DTN epoch).
func DTNTime(t time.Time) uint64 {
	ms := t.UTC().Sub(dtnEpoch).Milliseconds()
	if ms < 0 {
		return 0
	}
	return uint64(ms)
}

// FromDTNTime converts a DTN time back to a wall-clock instant in UTC.
func FromDTNTime(ms uint64) time.Time {
	return dtnEpoch.Add(time.Duration(ms) * time.Millisecond).UTC()
}

// CreationTimestamp identifies when a bundle was created. Together with the
// source endpoint (and, for fragments, the fragment offset and length) it
// uniquely identifies a bundle. The sequence number disambiguates bundles
// created by the same source within a single millisecond.
type CreationTimestamp struct {
	// Time is the DTN creation time in milliseconds. A value of zero means the
	// source node lacked an accurate clock; a bundle-age block should then be
	// used to track elapsed lifetime.
	Time uint64
	// SequenceNumber counts bundles created at the same Time by the same source.
	SequenceNumber uint64
}

// crcrc32c returns the CRC-32C (Castagnoli) checksum used by CRC type 2.
var castagnoli = crc32.MakeTable(crc32.Castagnoli)

func crc32c(data []byte) uint32 {
	return crc32.Checksum(data, castagnoli)
}

// crc16X25 computes the CRC-16/X-25 checksum used by CRC type 1: polynomial
// 0x1021, reflected input and output, initial value 0xFFFF, final XOR 0xFFFF.
func crc16X25(data []byte) uint16 {
	crc := uint16(0xFFFF)
	for _, b := range data {
		crc ^= uint16(b)
		for i := 0; i < 8; i++ {
			if crc&1 != 0 {
				crc = (crc >> 1) ^ 0x8408
			} else {
				crc >>= 1
			}
		}
	}
	return crc ^ 0xFFFF
}
