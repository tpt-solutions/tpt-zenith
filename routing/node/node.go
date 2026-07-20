// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Package node implements a store-and-forward Bundle Protocol node. A node
// accepts bundles (from a local application or from a peer), delivers those
// addressed to itself, and stores the rest until a contact-graph route allows
// them to be forwarded onward. This is the delay-tolerant "store-and-forward"
// behavior that lets data cross an intermittently connected satellite mesh.
package node

import (
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
)

// IngestResult describes what a node did with a bundle handed to Ingest.
type IngestResult int

const (
	// ResultDelivered means the bundle's destination is this node; it was
	// delivered to the local application.
	ResultDelivered IngestResult = iota
	// ResultStored means the bundle was queued for later forwarding.
	ResultStored
	// ResultDuplicate means the node has already seen this bundle.
	ResultDuplicate
	// ResultExpired means the bundle's lifetime had elapsed on arrival.
	ResultExpired
)

func (r IngestResult) String() string {
	switch r {
	case ResultDelivered:
		return "delivered"
	case ResultStored:
		return "stored"
	case ResultDuplicate:
		return "duplicate"
	case ResultExpired:
		return "expired"
	default:
		return "unknown"
	}
}

// Node is a single store-and-forward DTN node identified by a node ID such as
// "dtn://sat-7".
type Node struct {
	ID   string
	Plan *cgr.ContactPlan

	store     map[string]*bundle.Bundle
	order     []string
	delivered []*bundle.Bundle
	seen      map[string]bool
}

// New creates a node with the given ID and contact plan.
func New(id string, plan *cgr.ContactPlan) *Node {
	return &Node{
		ID:    id,
		Plan:  plan,
		store: map[string]*bundle.Bundle{},
		seen:  map[string]bool{},
	}
}

// Ingest accepts a bundle at time now. Bundles addressed to this node are
// delivered locally; others are stored for forwarding. Duplicates and expired
// bundles are discarded.
func (n *Node) Ingest(b *bundle.Bundle, now time.Time) IngestResult {
	id := b.ID()
	if n.seen[id] {
		return ResultDuplicate
	}
	n.seen[id] = true

	if b.Expired(now) {
		return ResultExpired
	}

	if b.Primary.Destination.NodeID() == n.ID {
		n.delivered = append(n.delivered, b)
		return ResultDelivered
	}

	n.store[id] = b
	n.order = append(n.order, id)
	return ResultStored
}

// Route computes the onward route for a bundle from this node at time now.
func (n *Node) Route(b *bundle.Bundle, now time.Time) (cgr.Route, bool) {
	if n.Plan == nil {
		return cgr.Route{}, false
	}
	dest := b.Primary.Destination.NodeID()
	return n.Plan.FindRoute(n.ID, dest, bundleSize(b), now)
}

// Dequeue removes a stored bundle from the forwarding queue, typically after it
// has been successfully forwarded to the next hop.
func (n *Node) Dequeue(bundleID string) {
	if _, ok := n.store[bundleID]; !ok {
		return
	}
	delete(n.store, bundleID)
	for i, id := range n.order {
		if id == bundleID {
			n.order = append(n.order[:i], n.order[i+1:]...)
			break
		}
	}
}

// Stored returns the bundles currently queued for forwarding, in the order they
// were stored.
func (n *Node) Stored() []*bundle.Bundle {
	out := make([]*bundle.Bundle, 0, len(n.order))
	for _, id := range n.order {
		out = append(out, n.store[id])
	}
	return out
}

// Delivered returns the bundles delivered to the local application, in order of
// delivery.
func (n *Node) Delivered() []*bundle.Bundle {
	return n.delivered
}

// bundleSize returns the on-the-wire size of a bundle in bytes, used as the
// transfer volume for contact scheduling. It falls back to the payload length
// if serialization fails.
func bundleSize(b *bundle.Bundle) int {
	if enc, err := b.Marshal(); err == nil {
		return len(enc)
	}
	return len(b.Payload())
}
