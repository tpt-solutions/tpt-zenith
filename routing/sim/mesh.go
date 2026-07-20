// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Package sim provides a discrete-event simulation of a multi-node DTN mesh:
// satellite and ground-station nodes connected by intermittent links described
// by a contact plan. Bundles injected at a source node are stored, forwarded,
// and delivered across the mesh according to contact-graph routing, exercising
// the full delay-tolerant store-and-forward path end to end.
package sim

import (
	"container/heap"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
	"github.com/TPT-Solutions/tpt-zenith/routing/node"
)

// Mesh is a simulated network of DTN nodes sharing a single contact plan.
type Mesh struct {
	nodes    map[string]*node.Node
	plan     *cgr.ContactPlan
	pending  []injection
	handlers map[string]AppHandler
}

// OutboundBundle is a bundle an application handler wants to introduce into the
// mesh in response to a delivery, originating at FromNode no earlier than At.
type OutboundBundle struct {
	FromNode string
	Bundle   *bundle.Bundle
	At       time.Time
}

// AppHandler is an application callback invoked when a bundle is delivered to a
// node that has one registered. It may return zero or more reply bundles to be
// routed back through the mesh. This is what lets a satellite compute node run
// on-orbit inference and return results over the DTN layer.
type AppHandler func(deliveredTo string, b *bundle.Bundle, now time.Time) []OutboundBundle

// NewMesh creates a mesh backed by the given contact plan.
func NewMesh(plan *cgr.ContactPlan) *Mesh {
	return &Mesh{nodes: map[string]*node.Node{}, plan: plan}
}

// SetHandler registers an application handler for a node. The node is created if
// it does not yet exist.
func (m *Mesh) SetHandler(nodeID string, h AppHandler) {
	if m.handlers == nil {
		m.handlers = map[string]AppHandler{}
	}
	m.AddNode(nodeID)
	m.handlers[nodeID] = h
}

// AddNode registers a node with the given node ID (for example "dtn://sat-7").
// If the node already exists it is returned unchanged.
func (m *Mesh) AddNode(id string) *node.Node {
	if n, ok := m.nodes[id]; ok {
		return n
	}
	n := node.New(id, m.plan)
	m.nodes[id] = n
	return n
}

// Node returns the registered node with the given ID, or nil.
func (m *Mesh) Node(id string) *node.Node {
	return m.nodes[id]
}

// EventKind classifies a simulation trace event.
type EventKind string

const (
	EventInjected      EventKind = "injected"
	EventDelivered     EventKind = "delivered"
	EventForwarded     EventKind = "forwarded"
	EventStored        EventKind = "stored"
	EventDuplicate     EventKind = "duplicate"
	EventExpired       EventKind = "expired"
	EventUndeliverable EventKind = "undeliverable"
)

// Event is a single entry in the simulation trace.
type Event struct {
	Time     time.Time
	Node     string
	Kind     EventKind
	BundleID string
	// NextHop is set for forwarding events.
	NextHop string
}

// Delivery records a bundle delivered to its destination node.
type Delivery struct {
	BundleID string
	Node     string
	Time     time.Time
	Payload  []byte
}

// Report summarizes a simulation run.
type Report struct {
	Events     []Event
	Deliveries []Delivery
}

// DeliveredID reports whether a bundle with the given ID was delivered.
func (r Report) DeliveredID(id string) bool {
	for _, d := range r.Deliveries {
		if d.BundleID == id {
			return true
		}
	}
	return false
}

// injection is a bundle to introduce into the mesh at a given node and time.
type injection struct {
	nodeID string
	b      *bundle.Bundle
	at     time.Time
}

// Inject schedules a bundle to enter the mesh at nodeID at time at. The node is
// created if it does not yet exist.
func (m *Mesh) Inject(nodeID string, b *bundle.Bundle, at time.Time) {
	m.AddNode(nodeID)
	m.pending = append(m.pending, injection{nodeID: nodeID, b: b, at: at})
}

// maxEvents guards against pathological loops; delivery always terminates well
// under this bound for a finite contact plan.
const maxEvents = 1_000_000

// Run executes the discrete-event simulation until no further bundle movement
// is possible, returning a trace and the set of deliveries.
func (m *Mesh) Run() Report {
	var report Report

	eq := &eventQueue{}
	heap.Init(eq)
	for _, inj := range m.pending {
		heap.Push(eq, meshEvent{time: inj.at, nodeID: inj.nodeID, b: inj.b, injected: true})
	}

	processed := 0
	for eq.Len() > 0 {
		if processed++; processed > maxEvents {
			break
		}
		ev := heap.Pop(eq).(meshEvent)
		n := m.AddNode(ev.nodeID)

		if ev.injected {
			report.Events = append(report.Events, Event{
				Time: ev.time, Node: ev.nodeID, Kind: EventInjected, BundleID: ev.b.ID(),
			})
		}

		result := n.Ingest(ev.b, ev.time)
		switch result {
		case node.ResultDelivered:
			report.Events = append(report.Events, Event{
				Time: ev.time, Node: ev.nodeID, Kind: EventDelivered, BundleID: ev.b.ID(),
			})
			report.Deliveries = append(report.Deliveries, Delivery{
				BundleID: ev.b.ID(), Node: ev.nodeID, Time: ev.time, Payload: ev.b.Payload(),
			})
			if h := m.handlers[ev.nodeID]; h != nil {
				for _, ob := range h(ev.nodeID, ev.b, ev.time) {
					if ob.Bundle == nil {
						continue
					}
					when := ob.At
					if when.Before(ev.time) {
						when = ev.time
					}
					heap.Push(eq, meshEvent{time: when, nodeID: ob.FromNode, b: ob.Bundle, injected: true})
				}
			}
		case node.ResultDuplicate:
			report.Events = append(report.Events, Event{
				Time: ev.time, Node: ev.nodeID, Kind: EventDuplicate, BundleID: ev.b.ID(),
			})
		case node.ResultExpired:
			report.Events = append(report.Events, Event{
				Time: ev.time, Node: ev.nodeID, Kind: EventExpired, BundleID: ev.b.ID(),
			})
		case node.ResultStored:
			route, ok := n.Route(ev.b, ev.time)
			if !ok || len(route.Hops) == 0 {
				report.Events = append(report.Events, Event{
					Time: ev.time, Node: ev.nodeID, Kind: EventUndeliverable, BundleID: ev.b.ID(),
				})
				continue
			}
			hop := route.Hops[0]
			arrival, ok := hop.ArrivalTime(ev.time, bundleSize(ev.b))
			if !ok {
				report.Events = append(report.Events, Event{
					Time: ev.time, Node: ev.nodeID, Kind: EventUndeliverable, BundleID: ev.b.ID(),
				})
				continue
			}
			n.Dequeue(ev.b.ID())
			report.Events = append(report.Events, Event{
				Time: ev.time, Node: ev.nodeID, Kind: EventForwarded, BundleID: ev.b.ID(), NextHop: hop.To,
			})
			heap.Push(eq, meshEvent{time: arrival, nodeID: hop.To, b: ev.b})
		}
	}
	return report
}

func bundleSize(b *bundle.Bundle) int {
	if enc, err := b.Marshal(); err == nil {
		return len(enc)
	}
	return len(b.Payload())
}

// meshEvent is a scheduled bundle arrival at a node.
type meshEvent struct {
	time     time.Time
	nodeID   string
	b        *bundle.Bundle
	injected bool
}

type eventQueue []meshEvent

func (q eventQueue) Len() int           { return len(q) }
func (q eventQueue) Less(i, j int) bool { return q[i].time.Before(q[j].time) }
func (q eventQueue) Swap(i, j int)      { q[i], q[j] = q[j], q[i] }
func (q *eventQueue) Push(x interface{}) {
	*q = append(*q, x.(meshEvent))
}
func (q *eventQueue) Pop() interface{} {
	old := *q
	n := len(old)
	item := old[n-1]
	*q = old[:n-1]
	return item
}
