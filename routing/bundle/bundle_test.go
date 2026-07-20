// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package bundle

import (
	"bytes"
	"testing"
	"time"
)

func TestEIDRoundTrip(t *testing.T) {
	cases := []string{
		"dtn://ground-1/inbox",
		"dtn:none",
		"ipn:5.1",
		"ipn:100.0",
	}
	for _, uri := range cases {
		eid, err := ParseEID(uri)
		if err != nil {
			t.Fatalf("ParseEID(%q): %v", uri, err)
		}
		if got := eid.String(); got != uri {
			t.Fatalf("String() = %q, want %q", got, uri)
		}
		back, err := eidFromCBOR(eid.toCBOR())
		if err != nil {
			t.Fatalf("eidFromCBOR(%q): %v", uri, err)
		}
		if back != eid {
			t.Fatalf("cbor round-trip: got %#v want %#v", back, eid)
		}
	}
}

func TestNodeID(t *testing.T) {
	cases := map[string]string{
		"dtn://sat-7/telemetry": "dtn://sat-7",
		"dtn://ground-1/inbox":  "dtn://ground-1",
		"ipn:42.9":              "ipn:42",
	}
	for uri, want := range cases {
		eid := MustParseEID(uri)
		if got := eid.NodeID(); got != want {
			t.Fatalf("NodeID(%q) = %q, want %q", uri, got, want)
		}
	}
}

func TestDTNTimeRoundTrip(t *testing.T) {
	when := time.Date(2026, 7, 21, 10, 49, 38, 0, time.UTC)
	ms := DTNTime(when)
	back := FromDTNTime(ms)
	if !back.Equal(when) {
		t.Fatalf("DTN time round-trip: got %v want %v", back, when)
	}
}

func TestBundleMarshalRoundTrip(t *testing.T) {
	src := MustParseEID("dtn://sat-7/telemetry")
	dst := MustParseEID("dtn://ground-1/inbox")
	payload := []byte("hello orbital edge")
	b := NewBundle(src, dst, payload, Options{
		Lifetime:       2 * time.Hour,
		CreationTime:   1234567,
		SequenceNumber: 3,
	})

	enc, err := b.Marshal()
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}
	// A BPv7 bundle is an indefinite-length CBOR array: starts 0x9f, ends 0xff.
	if enc[0] != 0x9f {
		t.Fatalf("bundle does not start with indefinite-array header, got 0x%02x", enc[0])
	}
	if enc[len(enc)-1] != 0xff {
		t.Fatalf("bundle does not end with break, got 0x%02x", enc[len(enc)-1])
	}

	got, err := Unmarshal(enc)
	if err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}
	if got.Primary.Source != src || got.Primary.Destination != dst {
		t.Fatalf("endpoint mismatch: %+v", got.Primary)
	}
	if got.Primary.CreationTime.Time != 1234567 || got.Primary.CreationTime.SequenceNumber != 3 {
		t.Fatalf("creation timestamp mismatch: %+v", got.Primary.CreationTime)
	}
	if !bytes.Equal(got.Payload(), payload) {
		t.Fatalf("payload mismatch: got %q want %q", got.Payload(), payload)
	}
	if got.ID() != b.ID() {
		t.Fatalf("id mismatch: got %q want %q", got.ID(), b.ID())
	}
}

func TestCRCDetectsCorruption(t *testing.T) {
	b := NewBundle(
		MustParseEID("ipn:1.1"),
		MustParseEID("ipn:2.1"),
		[]byte("integrity"),
		Options{CreationTime: 10, PayloadCRCType: CRC32},
	)
	enc, err := b.Marshal()
	if err != nil {
		t.Fatal(err)
	}
	// Flip a bit inside the payload region and expect CRC verification to fail.
	corrupt := append([]byte(nil), enc...)
	idx := bytes.Index(corrupt, []byte("integrity"))
	if idx < 0 {
		t.Fatal("payload not found in encoding")
	}
	corrupt[idx] ^= 0xff
	if _, err := Unmarshal(corrupt); err == nil {
		t.Fatal("expected CRC verification to fail on corrupted payload")
	}
}

func TestCRC16Path(t *testing.T) {
	b := NewBundle(
		MustParseEID("dtn://a/x"),
		MustParseEID("dtn://b/y"),
		[]byte("crc16"),
		Options{CreationTime: 5, CRCType: CRC16, PayloadCRCType: CRC16},
	)
	enc, err := b.Marshal()
	if err != nil {
		t.Fatal(err)
	}
	got, err := Unmarshal(enc)
	if err != nil {
		t.Fatalf("Unmarshal with CRC16: %v", err)
	}
	if !bytes.Equal(got.Payload(), []byte("crc16")) {
		t.Fatalf("payload mismatch under CRC16")
	}
}

func TestExpired(t *testing.T) {
	created := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	b := NewBundle(
		MustParseEID("dtn://a/x"),
		MustParseEID("dtn://b/y"),
		[]byte("x"),
		Options{Lifetime: time.Minute, CreationTime: DTNTime(created)},
	)
	if b.Expired(created.Add(30 * time.Second)) {
		t.Fatal("bundle expired early")
	}
	if !b.Expired(created.Add(2 * time.Minute)) {
		t.Fatal("bundle should have expired")
	}
}
