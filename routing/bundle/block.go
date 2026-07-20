// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package bundle

import "fmt"

// CRC type code points (RFC 9171 section 4.2.1).
const (
	CRCNone uint64 = 0
	CRC16   uint64 = 1 // CRC-16/X-25
	CRC32   uint64 = 2 // CRC-32C (Castagnoli)
)

// Block type codes (RFC 9171 section 9.1 and section 4.4.x).
const (
	BlockTypePayload      uint64 = 1
	BlockTypePreviousNode uint64 = 6
	BlockTypeBundleAge    uint64 = 7
	BlockTypeHopCount     uint64 = 10
)

// The payload block is always block number 1 (RFC 9171 section 4.3.3).
const PayloadBlockNumber uint64 = 1

// Bundle processing control flags (RFC 9171 section 4.2.3).
const (
	FlagIsFragment          uint64 = 1 << 0
	FlagAdminRecord         uint64 = 1 << 1
	FlagMustNotFragment     uint64 = 1 << 2
	FlagAppAckRequested     uint64 = 1 << 5
	FlagStatusTimeRequested uint64 = 1 << 6
	FlagReportReception     uint64 = 1 << 14
	FlagReportForwarding    uint64 = 1 << 16
	FlagReportDelivery      uint64 = 1 << 17
	FlagReportDeletion      uint64 = 1 << 18
)

// Block processing control flags (RFC 9171 section 4.2.4).
const (
	BlockFlagReplicateInEveryFragment  uint64 = 1 << 0
	BlockFlagReportIfUnprocessed       uint64 = 1 << 1
	BlockFlagDeleteBundleIfUnprocessed uint64 = 1 << 2
	BlockFlagDiscardBlockIfUnprocessed uint64 = 1 << 4
)

// PrimaryBlock is the first block of every bundle (RFC 9171 section 4.3.1).
type PrimaryBlock struct {
	Version      uint64
	Flags        uint64
	CRCType      uint64
	Destination  EndpointID
	Source       EndpointID
	ReportTo     EndpointID
	CreationTime CreationTimestamp
	// Lifetime is the bundle's permitted lifespan in milliseconds, measured
	// from its creation time.
	Lifetime uint64

	// Fragmentation fields, present only when FlagIsFragment is set.
	FragmentOffset uint64
	TotalADULength uint64
}

// IsFragment reports whether the fragmentation flag is set.
func (p PrimaryBlock) IsFragment() bool {
	return p.Flags&FlagIsFragment != 0
}

// CanonicalBlock is any non-primary block, including the payload block
// (RFC 9171 section 4.3.2).
type CanonicalBlock struct {
	Type    uint64
	Number  uint64
	Flags   uint64
	CRCType uint64
	// Data is the block-type-specific data. For a payload block it is the
	// application data unit itself.
	Data []byte
}

// crcLen returns the CRC field length in bytes for a CRC type.
func crcLen(crcType uint64) (int, error) {
	switch crcType {
	case CRCNone:
		return 0, nil
	case CRC16:
		return 2, nil
	case CRC32:
		return 4, nil
	default:
		return 0, fmt.Errorf("bundle: unknown CRC type %d", crcType)
	}
}

// asUint64 coerces a decoded CBOR value to uint64, accepting the non-negative
// int64 that a small negative-looking value never produces here.
func asUint64(v interface{}) (uint64, error) {
	switch x := v.(type) {
	case uint64:
		return x, nil
	case int64:
		if x < 0 {
			return 0, fmt.Errorf("bundle: negative value %d where unsigned expected", x)
		}
		return uint64(x), nil
	default:
		return 0, fmt.Errorf("bundle: expected unsigned integer, got %T", v)
	}
}
