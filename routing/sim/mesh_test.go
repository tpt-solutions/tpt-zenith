// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package sim

import (
	"testing"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
)

func at(base time.Time, min float64) time.Time {
	return base.Add(time.Duration(min * float64(time.Minute)))
}

// TestStoreAndForwardDelivery exercises the canonical delay-tolerant scenario:
// a ground station hands a bundle to a satellite, the satellite carries it
// on-orbit across a coverage gap, then downlinks it to a second, initially
// unreachable ground station.
func TestStoreAndForwardDelivery(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	plan := cgr.NewContactPlan([]cgr.Contact{
		{From: "dtn://ground-1", To: "dtn://sat-7", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, OWLT: 3 * time.Millisecond, Confidence: 1},
		{From: "dtn://sat-7", To: "dtn://ground-2", Start: at(base, 55), End: at(base, 60), DataRate: 1e6, OWLT: 3 * time.Millisecond, Confidence: 1},
	})
	m := NewMesh(plan)

	b := bundle.NewBundle(
		bundle.MustParseEID("dtn://ground-1/out"),
		bundle.MustParseEID("dtn://ground-2/inbox"),
		[]byte("earth-observation result"),
		bundle.Options{Lifetime: 4 * time.Hour, CreationTime: bundle.DTNTime(base)},
	)
	m.Inject("dtn://ground-1", b, base)

	report := m.Run()

	if !report.DeliveredID(b.ID()) {
		t.Fatalf("bundle %s was not delivered; events=%+v", b.ID(), report.Events)
	}
	if len(report.Deliveries) != 1 {
		t.Fatalf("expected exactly one delivery, got %d", len(report.Deliveries))
	}
	d := report.Deliveries[0]
	if d.Node != "dtn://ground-2" {
		t.Fatalf("delivered to %q, want dtn://ground-2", d.Node)
	}
	if string(d.Payload) != "earth-observation result" {
		t.Fatalf("payload corrupted: %q", d.Payload)
	}
	// Delivery must occur no earlier than the downlink window opens.
	if d.Time.Before(at(base, 55)) {
		t.Fatalf("delivered at %v, before downlink window", d.Time)
	}

	// The satellite must actually have relayed (forwarded) the bundle.
	var forwardedBySat bool
	for _, e := range report.Events {
		if e.Kind == EventForwarded && e.Node == "dtn://sat-7" {
			forwardedBySat = true
		}
	}
	if !forwardedBySat {
		t.Fatal("expected the satellite to forward the bundle")
	}
}

func TestUndeliverableWhenNoDownlink(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	// Uplink to the satellite exists, but the satellite never sees ground-2.
	plan := cgr.NewContactPlan([]cgr.Contact{
		{From: "dtn://ground-1", To: "dtn://sat-7", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, Confidence: 1},
	})
	m := NewMesh(plan)
	b := bundle.NewBundle(
		bundle.MustParseEID("dtn://ground-1/out"),
		bundle.MustParseEID("dtn://ground-2/inbox"),
		[]byte("no way home"),
		bundle.Options{Lifetime: time.Hour, CreationTime: bundle.DTNTime(base)},
	)
	m.Inject("dtn://ground-1", b, base)
	report := m.Run()
	if report.DeliveredID(b.ID()) {
		t.Fatal("did not expect delivery without a downlink contact")
	}
}

func TestMultiHopRelay(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	// Two-satellite relay with an inter-satellite link in the middle.
	plan := cgr.NewContactPlan([]cgr.Contact{
		{From: "dtn://ground-1", To: "dtn://sat-a", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, Confidence: 1},
		{From: "dtn://sat-a", To: "dtn://sat-b", Start: at(base, 20), End: at(base, 25), DataRate: 1e6, Confidence: 1},
		{From: "dtn://sat-b", To: "dtn://ground-2", Start: at(base, 50), End: at(base, 55), DataRate: 1e6, Confidence: 1},
	})
	m := NewMesh(plan)
	b := bundle.NewBundle(
		bundle.MustParseEID("dtn://ground-1/out"),
		bundle.MustParseEID("dtn://ground-2/inbox"),
		[]byte("via two sats"),
		bundle.Options{Lifetime: 4 * time.Hour, CreationTime: bundle.DTNTime(base)},
	)
	m.Inject("dtn://ground-1", b, base)
	report := m.Run()
	if !report.DeliveredID(b.ID()) {
		t.Fatalf("multi-hop delivery failed; events=%+v", report.Events)
	}
}
