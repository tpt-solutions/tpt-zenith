// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package cbor

import (
	"bytes"
	"reflect"
	"testing"
)

func TestMarshalKnownVectors(t *testing.T) {
	// Vectors from RFC 8949 Appendix A.
	cases := []struct {
		name string
		in   interface{}
		want []byte
	}{
		{"zero", uint64(0), []byte{0x00}},
		{"ten", uint64(10), []byte{0x0a}},
		{"twenty-three", uint64(23), []byte{0x17}},
		{"twenty-four", uint64(24), []byte{0x18, 0x18}},
		{"hundred", uint64(100), []byte{0x18, 0x64}},
		{"thousand", uint64(1000), []byte{0x19, 0x03, 0xe8}},
		{"million", uint64(1000000), []byte{0x1a, 0x00, 0x0f, 0x42, 0x40}},
		{"neg-one", int64(-1), []byte{0x20}},
		{"neg-hundred", int64(-100), []byte{0x38, 0x63}},
		{"empty-bytes", []byte{}, []byte{0x40}},
		{"bytes-1234", []byte{1, 2, 3, 4}, []byte{0x44, 0x01, 0x02, 0x03, 0x04}},
		{"empty-text", "", []byte{0x60}},
		{"text-a", "a", []byte{0x61, 0x61}},
		{"text-IETF", "IETF", []byte{0x64, 0x49, 0x45, 0x54, 0x46}},
		{"empty-array", []interface{}{}, []byte{0x80}},
		{"array-123", []interface{}{uint64(1), uint64(2), uint64(3)}, []byte{0x83, 0x01, 0x02, 0x03}},
		{"false", false, []byte{0xf4}},
		{"true", true, []byte{0xf5}},
		{"null", nil, []byte{0xf6}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, err := Marshal(tc.in)
			if err != nil {
				t.Fatalf("Marshal(%v): %v", tc.in, err)
			}
			if !bytes.Equal(got, tc.want) {
				t.Fatalf("Marshal(%v) = % x, want % x", tc.in, got, tc.want)
			}
		})
	}
}

func TestIndefiniteArray(t *testing.T) {
	got, err := Marshal(IndefArray{uint64(1), uint64(2)})
	if err != nil {
		t.Fatal(err)
	}
	want := []byte{0x9f, 0x01, 0x02, 0xff}
	if !bytes.Equal(got, want) {
		t.Fatalf("indefinite array = % x, want % x", got, want)
	}
	// It decodes back to a plain array.
	v, n, err := Unmarshal(got)
	if err != nil {
		t.Fatal(err)
	}
	if n != len(got) {
		t.Fatalf("consumed %d bytes, want %d", n, len(got))
	}
	arr, ok := v.([]interface{})
	if !ok || len(arr) != 2 {
		t.Fatalf("decoded %#v, want 2-element array", v)
	}
}

func TestRoundTrip(t *testing.T) {
	values := []interface{}{
		uint64(0),
		uint64(255),
		uint64(65535),
		uint64(4294967295),
		int64(-1),
		int64(-1000000),
		[]byte("payload bytes"),
		"dtn://node/service",
		[]interface{}{uint64(7), "abc", []byte{9, 9}, []interface{}{uint64(1)}},
	}
	for _, v := range values {
		enc, err := Marshal(v)
		if err != nil {
			t.Fatalf("Marshal(%#v): %v", v, err)
		}
		dec, n, err := Unmarshal(enc)
		if err != nil {
			t.Fatalf("Unmarshal(%#v): %v", v, err)
		}
		if n != len(enc) {
			t.Fatalf("consumed %d of %d bytes for %#v", n, len(enc), v)
		}
		if !reflect.DeepEqual(v, dec) {
			t.Fatalf("round-trip mismatch: got %#v want %#v", dec, v)
		}
	}
}

func TestUnmarshalTruncated(t *testing.T) {
	// 0x18 announces a 1-byte argument that is missing.
	if _, _, err := Unmarshal([]byte{0x18}); err == nil {
		t.Fatal("expected error on truncated input")
	}
}
