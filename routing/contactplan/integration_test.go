// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package contactplan_test

import (
	"path/filepath"
	"testing"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/contactplan"
	"github.com/TPT-Solutions/tpt-zenith/routing/sim"
)

// TestVisibilityWindowsDriveDelivery is the Phase 1 -> Phase 2 end-to-end
// integration test. It loads a contact plan exported from the orbital-mechanics
// engine's visibility-window calculation (see the export_contacts tool) and
// confirms that a bundle can be delay-tolerantly routed from one ground station
// to another via a satellite relay, using only those computed contacts.
//
// The fixture describes an ISS pass geometry: the satellite sees Tokyo and
// Kauai at different, non-overlapping times, so delivery is only possible by
// storing the bundle on-orbit across the coverage gap.
func TestVisibilityWindowsDriveDelivery(t *testing.T) {
	sched, err := contactplan.LoadFile(filepath.Join("testdata", "contacts.json"))
	if err != nil {
		t.Fatalf("load contact plan: %v", err)
	}

	m := sim.NewMesh(sched.Plan)

	// Create the payload at the moment of the epoch and route it Tokyo -> Kauai.
	b := bundle.NewBundle(
		bundle.MustParseEID("dtn://ground-tokyo/out"),
		bundle.MustParseEID("dtn://ground-kauai/inbox"),
		[]byte("on-orbit inference result"),
		bundle.Options{
			Lifetime:     24 * time.Hour,
			CreationTime: bundle.DTNTime(sched.Epoch),
		},
	)
	m.Inject("dtn://ground-tokyo", b, sched.Epoch)

	report := m.Run()

	if !report.DeliveredID(b.ID()) {
		t.Fatalf("bundle not delivered end-to-end; events=%+v", report.Events)
	}

	var d sim.Delivery
	for _, dv := range report.Deliveries {
		if dv.BundleID == b.ID() {
			d = dv
		}
	}
	if d.Node != "dtn://ground-kauai" {
		t.Fatalf("delivered to %q, want dtn://ground-kauai", d.Node)
	}
	if string(d.Payload) != "on-orbit inference result" {
		t.Fatalf("payload mismatch: %q", d.Payload)
	}

	// The delivery must route through the satellite relay, and the satellite
	// must have stored the bundle across a genuine coverage gap (delivery well
	// after the initial uplink).
	var relayed bool
	for _, e := range report.Events {
		if e.Kind == sim.EventForwarded && e.Node == "dtn://sat-25544" {
			relayed = true
		}
	}
	if !relayed {
		t.Fatal("expected the satellite to relay the bundle")
	}

	gap := d.Time.Sub(sched.Epoch)
	if gap < time.Hour {
		t.Fatalf("expected a delay-tolerant (store-and-forward) delivery gap, got %v", gap)
	}
	t.Logf("delivered via satellite relay after %s of store-and-forward", gap.Round(time.Second))
}
