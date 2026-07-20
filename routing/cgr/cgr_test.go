// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package cgr

import (
	"testing"
	"time"
)

func at(base time.Time, min float64) time.Time {
	return base.Add(time.Duration(min * float64(time.Minute)))
}

func TestDirectContact(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	plan := NewContactPlan([]Contact{
		{From: "gs-a", To: "sat", Start: at(base, 0), End: at(base, 10), DataRate: 1e6, Confidence: 1},
	})
	route, ok := plan.FindRoute("gs-a", "sat", 1000, base)
	if !ok {
		t.Fatal("expected a route")
	}
	if len(route.Hops) != 1 {
		t.Fatalf("expected 1 hop, got %d", len(route.Hops))
	}
	if nh, _ := route.NextHop(); nh != "sat" {
		t.Fatalf("next hop = %q, want sat", nh)
	}
}

func TestStoreAndForwardRelay(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	// A satellite is visible to ground station A early, then (after storing the
	// bundle on-orbit) visible to ground station B later. There is no direct
	// A->B contact, so delivery must be delay-tolerant via the satellite.
	plan := NewContactPlan([]Contact{
		{From: "gs-a", To: "sat", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, Confidence: 1},
		{From: "sat", To: "gs-b", Start: at(base, 40), End: at(base, 45), DataRate: 1e6, Confidence: 1},
	})
	route, ok := plan.FindRoute("gs-a", "gs-b", 1000, base)
	if !ok {
		t.Fatal("expected a delay-tolerant route via the satellite")
	}
	if len(route.Hops) != 2 {
		t.Fatalf("expected 2 hops, got %d", len(route.Hops))
	}
	if route.Hops[0].To != "sat" || route.Hops[1].To != "gs-b" {
		t.Fatalf("unexpected hop sequence: %+v", route.Hops)
	}
	// Delivery cannot happen before the second contact opens.
	if route.ArrivalTime.Before(at(base, 40)) {
		t.Fatalf("arrival %v earlier than second contact start", route.ArrivalTime)
	}
}

func TestNoRouteWhenContactClosed(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	// The only contact ends before the bundle is ready.
	plan := NewContactPlan([]Contact{
		{From: "gs-a", To: "sat", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, Confidence: 1},
	})
	if _, ok := plan.FindRoute("gs-a", "sat", 1000, at(base, 10)); ok {
		t.Fatal("expected no route: contact already closed")
	}
}

func TestChoosesEarliestArrival(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	// Two relays: sat-fast opens later windows but yields earlier delivery.
	plan := NewContactPlan([]Contact{
		{From: "gs-a", To: "sat-slow", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, Confidence: 1},
		{From: "sat-slow", To: "gs-b", Start: at(base, 90), End: at(base, 95), DataRate: 1e6, Confidence: 1},
		{From: "gs-a", To: "sat-fast", Start: at(base, 0), End: at(base, 5), DataRate: 1e6, Confidence: 1},
		{From: "sat-fast", To: "gs-b", Start: at(base, 30), End: at(base, 35), DataRate: 1e6, Confidence: 1},
	})
	route, ok := plan.FindRoute("gs-a", "gs-b", 1000, base)
	if !ok {
		t.Fatal("expected a route")
	}
	if route.Hops[0].To != "sat-fast" {
		t.Fatalf("expected earliest-arrival via sat-fast, got %q", route.Hops[0].To)
	}
}

func TestCapacityTooSmall(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	// 1-second window at 1000 B/s = 1000 B capacity; a 5000 B bundle cannot fit.
	plan := NewContactPlan([]Contact{
		{From: "gs-a", To: "sat", Start: at(base, 0), End: base.Add(time.Second), DataRate: 1000, Confidence: 1},
	})
	if _, ok := plan.FindRoute("gs-a", "sat", 5000, base); ok {
		t.Fatal("expected no route: contact capacity too small for bundle")
	}
}
