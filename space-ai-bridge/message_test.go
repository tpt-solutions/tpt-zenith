// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import (
	"bytes"
	"reflect"
	"testing"
)

func TestMessageRoundTrip(t *testing.T) {
	msgs := []Message{
		ModelUpload{ModelID: "ndvi-v1", Kind: "scene-stats", Params: []byte{200}},
		InferenceRequest{RequestID: "req-1", ModelID: "ndvi-v1", InputID: "scene-42"},
		InferenceResponse{
			RequestID:   "req-1",
			ModelID:     "ndvi-v1",
			OK:          true,
			Result:      []byte{1, 2, 3, 4},
			Summary:     "ok",
			InputBytes:  1048576,
			OutputBytes: 40,
		},
	}
	for _, m := range msgs {
		enc, err := Encode(m)
		if err != nil {
			t.Fatalf("Encode(%T): %v", m, err)
		}
		dec, err := Decode(enc)
		if err != nil {
			t.Fatalf("Decode(%T): %v", m, err)
		}
		if !reflect.DeepEqual(m, dec) {
			t.Fatalf("round-trip mismatch:\n got %#v\nwant %#v", dec, m)
		}
	}
}

func TestDecodeRejectsGarbage(t *testing.T) {
	if _, err := Decode([]byte{0xff, 0x00}); err == nil {
		t.Fatal("expected error decoding garbage")
	}
}

func TestBandwidthSavings(t *testing.T) {
	r := InferenceResponse{InputBytes: 1000, OutputBytes: 50}
	if got := r.BandwidthSavings(); got < 0.949 || got > 0.951 {
		t.Fatalf("savings = %.4f, want ~0.95", got)
	}
	// Unknown input size yields zero.
	if (InferenceResponse{OutputBytes: 10}).BandwidthSavings() != 0 {
		t.Fatal("expected zero savings when input size unknown")
	}
	// A result larger than input never reports negative savings.
	if (InferenceResponse{InputBytes: 10, OutputBytes: 100}).BandwidthSavings() != 0 {
		t.Fatal("expected clamped zero savings")
	}
}

func TestResponseEncodingIsCompact(t *testing.T) {
	// A response for a 1 MiB scene must itself be tiny.
	r := InferenceResponse{
		RequestID:   "r",
		ModelID:     "m",
		OK:          true,
		Result:      bytes.Repeat([]byte{7}, 40),
		Summary:     "scene-stats: n=1048576 ...",
		InputBytes:  1 << 20,
		OutputBytes: 40,
	}
	enc, err := Encode(r)
	if err != nil {
		t.Fatal(err)
	}
	if len(enc) > 256 {
		t.Fatalf("encoded response is %d bytes, expected < 256", len(enc))
	}
}
