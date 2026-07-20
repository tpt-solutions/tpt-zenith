// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package contactplan

import (
	"strings"
	"testing"
	"time"
)

const sample = `{
  "epoch": "2024-01-15T12:00:00Z",
  "satellite": "dtn://sat-25544",
  "contacts": [
    {"from": "dtn://ground-tokyo", "to": "dtn://sat-25544", "start_min": 10.0, "end_min": 16.0, "data_rate_bytes_per_sec": 125000.0, "owlt_ms": 2.5, "confidence": 1.0},
    {"from": "dtn://sat-25544", "to": "dtn://ground-kauai", "start_min": 100.0, "end_min": 105.0, "data_rate_bytes_per_sec": 125000.0, "owlt_ms": 3.0, "confidence": 1.0}
  ]
}`

func TestLoad(t *testing.T) {
	s, err := Load(strings.NewReader(sample))
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	wantEpoch := time.Date(2024, 1, 15, 12, 0, 0, 0, time.UTC)
	if !s.Epoch.Equal(wantEpoch) {
		t.Fatalf("epoch = %v, want %v", s.Epoch, wantEpoch)
	}
	contacts := s.Plan.Contacts()
	if len(contacts) != 2 {
		t.Fatalf("expected 2 contacts, got %d", len(contacts))
	}
	first := contacts[0]
	if first.From != "dtn://ground-tokyo" || first.To != "dtn://sat-25544" {
		t.Fatalf("unexpected first contact: %+v", first)
	}
	if !first.Start.Equal(s.At(10.0)) {
		t.Fatalf("start = %v, want %v", first.Start, s.At(10.0))
	}
	if first.OWLT != time.Duration(2.5*float64(time.Millisecond)) {
		t.Fatalf("owlt = %v", first.OWLT)
	}
}

func TestLoadRejectsBadEpoch(t *testing.T) {
	bad := `{"epoch": "not-a-time", "contacts": []}`
	if _, err := Load(strings.NewReader(bad)); err == nil {
		t.Fatal("expected error on bad epoch")
	}
}

func TestLoadRejectsInvertedWindow(t *testing.T) {
	bad := `{"epoch": "2024-01-15T12:00:00Z", "contacts": [
		{"from": "a", "to": "b", "start_min": 20.0, "end_min": 10.0}
	]}`
	if _, err := Load(strings.NewReader(bad)); err == nil {
		t.Fatal("expected error on inverted window")
	}
}
