// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package node

import (
	"testing"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
)

func mkBundle(src, dst string, payload string, created time.Time) *bundle.Bundle {
	return bundle.NewBundle(
		bundle.MustParseEID(src),
		bundle.MustParseEID(dst),
		[]byte(payload),
		bundle.Options{Lifetime: time.Hour, CreationTime: bundle.DTNTime(created)},
	)
}

func TestIngestLocalDelivery(t *testing.T) {
	now := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	n := New("dtn://ground-1", nil)
	b := mkBundle("dtn://sat-7/telemetry", "dtn://ground-1/inbox", "hi", now)
	if r := n.Ingest(b, now); r != ResultDelivered {
		t.Fatalf("expected delivered, got %s", r)
	}
	if len(n.Delivered()) != 1 {
		t.Fatalf("expected 1 delivered bundle, got %d", len(n.Delivered()))
	}
}

func TestIngestStoresForForwarding(t *testing.T) {
	now := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	n := New("dtn://sat-7", nil)
	b := mkBundle("dtn://ground-1/out", "dtn://ground-2/inbox", "relay me", now)
	if r := n.Ingest(b, now); r != ResultStored {
		t.Fatalf("expected stored, got %s", r)
	}
	if len(n.Stored()) != 1 {
		t.Fatalf("expected 1 stored bundle, got %d", len(n.Stored()))
	}
	n.Dequeue(b.ID())
	if len(n.Stored()) != 0 {
		t.Fatalf("expected empty store after dequeue, got %d", len(n.Stored()))
	}
}

func TestIngestDuplicate(t *testing.T) {
	now := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	n := New("dtn://sat-7", nil)
	b := mkBundle("dtn://ground-1/out", "dtn://ground-2/inbox", "x", now)
	_ = n.Ingest(b, now)
	if r := n.Ingest(b, now); r != ResultDuplicate {
		t.Fatalf("expected duplicate, got %s", r)
	}
}

func TestIngestExpired(t *testing.T) {
	created := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	n := New("dtn://sat-7", nil)
	b := bundle.NewBundle(
		bundle.MustParseEID("dtn://a/o"),
		bundle.MustParseEID("dtn://b/i"),
		[]byte("stale"),
		bundle.Options{Lifetime: time.Minute, CreationTime: bundle.DTNTime(created)},
	)
	if r := n.Ingest(b, created.Add(2*time.Minute)); r != ResultExpired {
		t.Fatalf("expected expired, got %s", r)
	}
}

func TestRouteUsesPlan(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	plan := cgr.NewContactPlan([]cgr.Contact{
		{From: "dtn://sat-7", To: "dtn://ground-2", Start: base, End: base.Add(10 * time.Minute), DataRate: 1e6, Confidence: 1},
	})
	n := New("dtn://sat-7", plan)
	b := mkBundle("dtn://ground-1/out", "dtn://ground-2/inbox", "payload", base)
	route, ok := n.Route(b, base)
	if !ok {
		t.Fatal("expected a route")
	}
	if nh, _ := route.NextHop(); nh != "dtn://ground-2" {
		t.Fatalf("next hop = %q, want dtn://ground-2", nh)
	}
}
