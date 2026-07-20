// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Package cgr implements contact-graph routing (CGR) for a delay-tolerant
// network. Routing decisions are driven by a contact plan: a schedule of future
// communication opportunities ("contacts") between nodes, such as the ground-
// station-to-satellite visibility windows produced by the orbital-mechanics
// engine. Given a contact plan, CGR computes the route (an ordered sequence of
// contacts) that delivers a bundle to its destination at the earliest possible
// time, accounting for intermittent links and propagation delay.
package cgr

import (
	"sort"
	"time"
)

// Contact is a scheduled, directed communication opportunity from one node to
// another. It is the fundamental edge of the contact graph.
type Contact struct {
	// From and To are node identifiers (see bundle.EndpointID.NodeID).
	From string
	To   string
	// Start and End bound the interval during which the link is usable.
	Start time.Time
	End   time.Time
	// DataRate is the link capacity in bytes per second. A non-positive rate is
	// treated as effectively instantaneous (unbounded) for scheduling.
	DataRate float64
	// OWLT is the one-way light time (propagation delay) across the link.
	OWLT time.Duration
	// Confidence is the probability (0..1] that the contact will occur. It is
	// carried for downstream policy use; the earliest-arrival search treats any
	// positive confidence as usable.
	Confidence float64
}

// Duration returns the wall-clock length of the contact.
func (c Contact) Duration() time.Duration {
	return c.End.Sub(c.Start)
}

// Volume is the maximum number of bytes that can traverse the contact over its
// whole interval. A non-positive data rate yields an effectively unbounded
// volume.
func (c Contact) Volume() float64 {
	if c.DataRate <= 0 {
		return 1e300
	}
	return c.DataRate * c.Duration().Seconds()
}

// ContactPlan is a set of contacts, indexed by their originating node for fast
// neighbor expansion during routing.
type ContactPlan struct {
	contacts []Contact
	byFrom   map[string][]int
}

// NewContactPlan builds a contact plan from a list of contacts.
func NewContactPlan(contacts []Contact) *ContactPlan {
	p := &ContactPlan{byFrom: map[string][]int{}}
	for _, c := range contacts {
		p.Add(c)
	}
	return p
}

// Add inserts a contact into the plan.
func (p *ContactPlan) Add(c Contact) {
	if p.byFrom == nil {
		p.byFrom = map[string][]int{}
	}
	idx := len(p.contacts)
	p.contacts = append(p.contacts, c)
	p.byFrom[c.From] = append(p.byFrom[c.From], idx)
}

// Contacts returns a copy of all contacts in the plan, sorted by start time.
func (p *ContactPlan) Contacts() []Contact {
	out := append([]Contact(nil), p.contacts...)
	sort.Slice(out, func(i, j int) bool {
		if out[i].Start.Equal(out[j].Start) {
			return out[i].From < out[j].From
		}
		return out[i].Start.Before(out[j].Start)
	})
	return out
}

// Nodes returns the set of node identifiers that appear in the plan.
func (p *ContactPlan) Nodes() []string {
	seen := map[string]bool{}
	for _, c := range p.contacts {
		seen[c.From] = true
		seen[c.To] = true
	}
	out := make([]string, 0, len(seen))
	for n := range seen {
		out = append(out, n)
	}
	sort.Strings(out)
	return out
}
