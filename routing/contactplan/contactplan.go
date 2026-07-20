// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Package contactplan loads DTN contact plans exported by the Phase 1 orbital-
// mechanics engine and converts them into the contact-graph the routing layer
// consumes. The on-disk format expresses contact windows in minutes after a
// propagation epoch (as produced by the engine's visibility-window
// calculation); this package resolves those offsets against the absolute epoch
// so the router can schedule store-and-forward routes.
package contactplan

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
)

// document is the JSON structure emitted by the orbital-mechanics
// export_contacts tool.
type document struct {
	Epoch     string        `json:"epoch"`
	Satellite string        `json:"satellite"`
	Contacts  []contactJSON `json:"contacts"`
}

type contactJSON struct {
	From             string  `json:"from"`
	To               string  `json:"to"`
	StartMin         float64 `json:"start_min"`
	EndMin           float64 `json:"end_min"`
	DataRateBytesSec float64 `json:"data_rate_bytes_per_sec"`
	OWLTms           float64 `json:"owlt_ms"`
	Confidence       float64 `json:"confidence"`
}

// Schedule is a loaded contact plan together with the absolute epoch its
// contact windows are measured from.
type Schedule struct {
	Epoch time.Time
	Plan  *cgr.ContactPlan
}

// Load parses a contact plan from r.
func Load(r io.Reader) (*Schedule, error) {
	var doc document
	if err := json.NewDecoder(r).Decode(&doc); err != nil {
		return nil, fmt.Errorf("contactplan: decode: %w", err)
	}
	epoch, err := time.Parse(time.RFC3339, doc.Epoch)
	if err != nil {
		return nil, fmt.Errorf("contactplan: parse epoch %q: %w", doc.Epoch, err)
	}

	contacts := make([]cgr.Contact, 0, len(doc.Contacts))
	for i, c := range doc.Contacts {
		if c.From == "" || c.To == "" {
			return nil, fmt.Errorf("contactplan: contact %d missing from/to", i)
		}
		if c.EndMin < c.StartMin {
			return nil, fmt.Errorf("contactplan: contact %d ends before it starts", i)
		}
		confidence := c.Confidence
		if confidence == 0 {
			confidence = 1.0
		}
		contacts = append(contacts, cgr.Contact{
			From:       c.From,
			To:         c.To,
			Start:      offset(epoch, c.StartMin),
			End:        offset(epoch, c.EndMin),
			DataRate:   c.DataRateBytesSec,
			OWLT:       time.Duration(c.OWLTms * float64(time.Millisecond)),
			Confidence: confidence,
		})
	}
	return &Schedule{Epoch: epoch, Plan: cgr.NewContactPlan(contacts)}, nil
}

// LoadFile parses a contact plan from a file on disk.
func LoadFile(path string) (*Schedule, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("contactplan: open %q: %w", path, err)
	}
	defer f.Close()
	return Load(f)
}

// At returns the absolute time corresponding to minutes after the schedule's
// epoch, convenient for injecting bundles relative to the plan.
func (s *Schedule) At(minutes float64) time.Time {
	return offset(s.Epoch, minutes)
}

func offset(epoch time.Time, minutes float64) time.Time {
	return epoch.Add(time.Duration(minutes * float64(time.Minute)))
}
