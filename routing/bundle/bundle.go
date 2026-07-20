// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Package bundle implements the Bundle Protocol version 7 (BPv7) primitives
// from RFC 9171: endpoint identifiers, creation timestamps, the primary block,
// canonical (including payload) blocks, and CRC-protected CBOR serialization of
// a complete bundle.
//
// The wire format is faithful to RFC 9171: a bundle is a CBOR indefinite-length
// array whose first element is the primary block and whose remaining elements
// are canonical blocks, each a definite-length CBOR array. CRC-16/X-25 and
// CRC-32C block integrity are supported.
package bundle

import (
	"encoding/binary"
	"fmt"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/cbor"
)

// Version is the Bundle Protocol version implemented here.
const Version uint64 = 7

// Bundle is a complete BPv7 bundle: a primary block followed by one or more
// canonical blocks. A well-formed bundle always contains exactly one payload
// block (type 1, number 1).
type Bundle struct {
	Primary   PrimaryBlock
	Canonical []CanonicalBlock
}

// Options configure NewBundle.
type Options struct {
	// Lifetime bounds how long the bundle remains valid. Defaults to 1 hour.
	Lifetime time.Duration
	// ReportTo receives status reports; defaults to the null endpoint.
	ReportTo EndpointID
	// Flags are additional bundle processing control flags to OR in.
	Flags uint64
	// CRCType selects primary-block CRC protection; defaults to CRC32.
	CRCType uint64
	// PayloadCRCType selects payload-block CRC protection; defaults to CRC32.
	PayloadCRCType uint64
	// CreationTime overrides the creation timestamp time (DTN ms). When zero,
	// the current wall-clock time is used.
	CreationTime uint64
	// SequenceNumber sets the creation-timestamp sequence number.
	SequenceNumber uint64
}

// NewBundle constructs a bundle carrying payload from source to destination.
func NewBundle(source, destination EndpointID, payload []byte, opts Options) *Bundle {
	if opts.Lifetime == 0 {
		opts.Lifetime = time.Hour
	}
	if opts.ReportTo == (EndpointID{}) {
		opts.ReportTo = NullEndpoint()
	}
	if opts.CRCType == 0 {
		opts.CRCType = CRC32
	}
	if opts.PayloadCRCType == 0 {
		opts.PayloadCRCType = CRC32
	}
	creation := opts.CreationTime
	if creation == 0 {
		creation = DTNTime(time.Now())
	}

	primary := PrimaryBlock{
		Version:     Version,
		Flags:       opts.Flags,
		CRCType:     opts.CRCType,
		Destination: destination,
		Source:      source,
		ReportTo:    opts.ReportTo,
		CreationTime: CreationTimestamp{
			Time:           creation,
			SequenceNumber: opts.SequenceNumber,
		},
		Lifetime: uint64(opts.Lifetime.Milliseconds()),
	}
	payloadBlock := CanonicalBlock{
		Type:    BlockTypePayload,
		Number:  PayloadBlockNumber,
		Flags:   0,
		CRCType: opts.PayloadCRCType,
		Data:    append([]byte(nil), payload...),
	}
	return &Bundle{Primary: primary, Canonical: []CanonicalBlock{payloadBlock}}
}

// Payload returns the application data unit from the payload block, or nil if
// the bundle has no payload block.
func (b *Bundle) Payload() []byte {
	for i := range b.Canonical {
		if b.Canonical[i].Type == BlockTypePayload {
			return b.Canonical[i].Data
		}
	}
	return nil
}

// ID returns a string that uniquely identifies the bundle by source, creation
// timestamp, and (for fragments) fragment offset and length. It is stable
// across serialization and is used by nodes to detect duplicates.
func (b *Bundle) ID() string {
	p := b.Primary
	id := fmt.Sprintf("%s|%d|%d", p.Source.String(), p.CreationTime.Time, p.CreationTime.SequenceNumber)
	if p.IsFragment() {
		id += fmt.Sprintf("|%d|%d", p.FragmentOffset, p.TotalADULength)
	}
	return id
}

// Expired reports whether the bundle's lifetime has elapsed as of now. Bundles
// created with a zero creation time (no accurate source clock) never expire by
// this check; a bundle-age block would be needed for those.
func (b *Bundle) Expired(now time.Time) bool {
	if b.Primary.CreationTime.Time == 0 {
		return false
	}
	created := FromDTNTime(b.Primary.CreationTime.Time)
	deadline := created.Add(time.Duration(b.Primary.Lifetime) * time.Millisecond)
	return now.After(deadline)
}

// Marshal serializes the bundle to its RFC 9171 CBOR representation.
func (b *Bundle) Marshal() ([]byte, error) {
	primaryFields, err := b.primaryFields()
	if err != nil {
		return nil, err
	}
	elems := cbor.IndefArray{primaryFields}
	for i := range b.Canonical {
		blockFields, err := canonicalFields(b.Canonical[i])
		if err != nil {
			return nil, err
		}
		elems = append(elems, blockFields)
	}
	return cbor.Marshal(elems)
}

func (b *Bundle) primaryFields() ([]interface{}, error) {
	p := b.Primary
	fields := []interface{}{
		p.Version,
		p.Flags,
		p.CRCType,
		p.Destination.toCBOR(),
		p.Source.toCBOR(),
		p.ReportTo.toCBOR(),
		[]interface{}{p.CreationTime.Time, p.CreationTime.SequenceNumber},
		p.Lifetime,
	}
	if p.IsFragment() {
		fields = append(fields, p.FragmentOffset, p.TotalADULength)
	}
	return appendCRC(fields, p.CRCType)
}

func canonicalFields(c CanonicalBlock) ([]interface{}, error) {
	fields := []interface{}{
		c.Type,
		c.Number,
		c.Flags,
		c.CRCType,
		append([]byte(nil), c.Data...),
	}
	return appendCRC(fields, c.CRCType)
}

// appendCRC appends the CRC field to a block's field list, computing the CRC
// over the CBOR encoding of the block with the CRC field zeroed, as required by
// RFC 9171 section 4.2.2.
func appendCRC(fields []interface{}, crcType uint64) ([]interface{}, error) {
	n, err := crcLen(crcType)
	if err != nil {
		return nil, err
	}
	if n == 0 {
		return fields, nil
	}
	fields = append(fields, make([]byte, n))
	zeroed, err := cbor.Marshal(fields)
	if err != nil {
		return nil, err
	}
	crcBytes := make([]byte, n)
	switch crcType {
	case CRC16:
		binary.BigEndian.PutUint16(crcBytes, crc16X25(zeroed))
	case CRC32:
		binary.BigEndian.PutUint32(crcBytes, crc32c(zeroed))
	}
	fields[len(fields)-1] = crcBytes
	return fields, nil
}

// Unmarshal parses a bundle from its RFC 9171 CBOR representation. It verifies
// each block's CRC.
func Unmarshal(data []byte) (*Bundle, error) {
	v, _, err := cbor.Unmarshal(data)
	if err != nil {
		return nil, fmt.Errorf("bundle: decode: %w", err)
	}
	blocks, ok := v.([]interface{})
	if !ok || len(blocks) < 2 {
		return nil, fmt.Errorf("bundle: expected an array of at least 2 blocks")
	}

	primaryArr, ok := blocks[0].([]interface{})
	if !ok {
		return nil, fmt.Errorf("bundle: primary block is not an array")
	}
	primary, err := parsePrimary(primaryArr)
	if err != nil {
		return nil, err
	}

	b := &Bundle{Primary: primary}
	for _, raw := range blocks[1:] {
		arr, ok := raw.([]interface{})
		if !ok {
			return nil, fmt.Errorf("bundle: canonical block is not an array")
		}
		cb, err := parseCanonical(arr)
		if err != nil {
			return nil, err
		}
		b.Canonical = append(b.Canonical, cb)
	}
	return b, nil
}

func parsePrimary(arr []interface{}) (PrimaryBlock, error) {
	var p PrimaryBlock
	if len(arr) < 8 {
		return p, fmt.Errorf("bundle: primary block has %d fields, need at least 8", len(arr))
	}
	var err error
	if p.Version, err = asUint64(arr[0]); err != nil {
		return p, err
	}
	if p.Flags, err = asUint64(arr[1]); err != nil {
		return p, err
	}
	if p.CRCType, err = asUint64(arr[2]); err != nil {
		return p, err
	}
	if p.Destination, err = eidFromCBOR(arr[3]); err != nil {
		return p, err
	}
	if p.Source, err = eidFromCBOR(arr[4]); err != nil {
		return p, err
	}
	if p.ReportTo, err = eidFromCBOR(arr[5]); err != nil {
		return p, err
	}
	ts, ok := arr[6].([]interface{})
	if !ok || len(ts) != 2 {
		return p, fmt.Errorf("bundle: creation timestamp must be a 2-element array")
	}
	if p.CreationTime.Time, err = asUint64(ts[0]); err != nil {
		return p, err
	}
	if p.CreationTime.SequenceNumber, err = asUint64(ts[1]); err != nil {
		return p, err
	}
	if p.Lifetime, err = asUint64(arr[7]); err != nil {
		return p, err
	}

	idx := 8
	if p.IsFragment() {
		if len(arr) < idx+2 {
			return p, fmt.Errorf("bundle: fragmented primary block missing offset/length")
		}
		if p.FragmentOffset, err = asUint64(arr[idx]); err != nil {
			return p, err
		}
		if p.TotalADULength, err = asUint64(arr[idx+1]); err != nil {
			return p, err
		}
		idx += 2
	}

	if p.CRCType != CRCNone {
		if len(arr) < idx+1 {
			return p, fmt.Errorf("bundle: primary block missing CRC field")
		}
		crcVal, ok := arr[idx].([]byte)
		if !ok {
			return p, fmt.Errorf("bundle: primary block CRC must be a byte string")
		}
		if err := verifyPrimaryCRC(p, crcVal); err != nil {
			return p, err
		}
	}
	return p, nil
}

func parseCanonical(arr []interface{}) (CanonicalBlock, error) {
	var c CanonicalBlock
	if len(arr) < 5 {
		return c, fmt.Errorf("bundle: canonical block has %d fields, need at least 5", len(arr))
	}
	var err error
	if c.Type, err = asUint64(arr[0]); err != nil {
		return c, err
	}
	if c.Number, err = asUint64(arr[1]); err != nil {
		return c, err
	}
	if c.Flags, err = asUint64(arr[2]); err != nil {
		return c, err
	}
	if c.CRCType, err = asUint64(arr[3]); err != nil {
		return c, err
	}
	data, ok := arr[4].([]byte)
	if !ok {
		return c, fmt.Errorf("bundle: canonical block data must be a byte string")
	}
	c.Data = data

	if c.CRCType != CRCNone {
		if len(arr) < 6 {
			return c, fmt.Errorf("bundle: canonical block missing CRC field")
		}
		crcVal, ok := arr[5].([]byte)
		if !ok {
			return c, fmt.Errorf("bundle: canonical block CRC must be a byte string")
		}
		expect, err := canonicalFields(c)
		if err != nil {
			return c, err
		}
		if err := verifyCRC(expect, c.CRCType, crcVal); err != nil {
			return c, fmt.Errorf("bundle: canonical block %d: %w", c.Number, err)
		}
	}
	return c, nil
}

func verifyPrimaryCRC(p PrimaryBlock, got []byte) error {
	b := &Bundle{Primary: p}
	fields, err := b.primaryFields()
	if err != nil {
		return err
	}
	if err := verifyCRC(fields, p.CRCType, got); err != nil {
		return fmt.Errorf("bundle: primary block: %w", err)
	}
	return nil
}

// verifyCRC recomputes the CRC over fields (whose last element already holds the
// authoritative CRC bytes) and compares it against got.
func verifyCRC(fields []interface{}, crcType uint64, got []byte) error {
	want, ok := fields[len(fields)-1].([]byte)
	if !ok {
		return fmt.Errorf("crc field is not bytes")
	}
	if len(want) != len(got) {
		return fmt.Errorf("crc length mismatch: got %d want %d", len(got), len(want))
	}
	for i := range want {
		if want[i] != got[i] {
			return fmt.Errorf("crc mismatch")
		}
	}
	return nil
}
