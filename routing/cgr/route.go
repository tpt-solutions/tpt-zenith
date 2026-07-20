// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package cgr

import (
	"container/heap"
	"time"
)

// Route is an ordered sequence of contacts that carries a bundle from a source
// node to a destination node, together with the resulting delivery time.
type Route struct {
	// Hops are the contacts to traverse, in order.
	Hops []Contact
	// ArrivalTime is the earliest time the bundle can arrive at the destination
	// by following Hops.
	ArrivalTime time.Time
}

// NextHop returns the To node of the first contact on the route, i.e. the node
// the bundle should be forwarded to next. It reports false for an empty route.
func (r Route) NextHop() (string, bool) {
	if len(r.Hops) == 0 {
		return "", false
	}
	return r.Hops[0].To, true
}

// FindRoute computes the earliest-arrival route from source to dest for a
// bundle of sizeBytes that becomes ready to send at readyAt. It performs a
// Dijkstra search over the contact graph (the CGR earliest-arrival strategy),
// respecting each contact's time window, capacity, and propagation delay.
//
// It returns the route and true if the destination is reachable, or a zero
// Route and false otherwise.
func (p *ContactPlan) FindRoute(source, dest string, sizeBytes int, readyAt time.Time) (Route, bool) {
	if source == dest {
		return Route{ArrivalTime: readyAt}, true
	}

	// best[node] is the earliest known arrival time at node.
	best := map[string]time.Time{source: readyAt}
	// pred[node] records the contact used to first reach node optimally.
	pred := map[string]Contact{}
	visited := map[string]bool{}

	pq := &pqueue{}
	heap.Init(pq)
	heap.Push(pq, pqItem{node: source, arrival: readyAt})

	for pq.Len() > 0 {
		cur := heap.Pop(pq).(pqItem)
		if visited[cur.node] {
			continue
		}
		visited[cur.node] = true

		if cur.node == dest {
			return reconstruct(pred, source, dest, cur.arrival), true
		}

		for _, idx := range p.byFrom[cur.node] {
			c := p.contacts[idx]
			arrival, ok := c.ArrivalTime(cur.arrival, sizeBytes)
			if !ok {
				continue
			}
			if prev, seen := best[c.To]; !seen || arrival.Before(prev) {
				best[c.To] = arrival
				pred[c.To] = c
				heap.Push(pq, pqItem{node: c.To, arrival: arrival})
			}
		}
	}
	return Route{}, false
}

// ArrivalTime computes the time a bundle present at c.From no earlier than
// readyAt would arrive at c.To across this contact, or reports false if the
// contact cannot carry it. It accounts for the contact window, transmission
// time, and propagation delay.
func (c Contact) ArrivalTime(readyAt time.Time, sizeBytes int) (time.Time, bool) {
	if c.Confidence < 0 {
		return time.Time{}, false
	}
	// Transmission can only begin once both the bundle is ready and the contact
	// has opened.
	txStart := readyAt
	if c.Start.After(txStart) {
		txStart = c.Start
	}
	if !txStart.Before(c.End) {
		return time.Time{}, false // contact already closed
	}
	var txDuration time.Duration
	if c.DataRate > 0 {
		seconds := float64(sizeBytes) / c.DataRate
		txDuration = time.Duration(seconds * float64(time.Second))
	}
	txFinish := txStart.Add(txDuration)
	if txFinish.After(c.End) {
		return time.Time{}, false // not enough time to transmit fully
	}
	return txFinish.Add(c.OWLT), true
}

func reconstruct(pred map[string]Contact, source, dest string, arrival time.Time) Route {
	var hops []Contact
	node := dest
	for node != source {
		c, ok := pred[node]
		if !ok {
			break
		}
		hops = append(hops, c)
		node = c.From
	}
	// Reverse into source->dest order.
	for i, j := 0, len(hops)-1; i < j; i, j = i+1, j-1 {
		hops[i], hops[j] = hops[j], hops[i]
	}
	return Route{Hops: hops, ArrivalTime: arrival}
}

// pqItem is a priority-queue entry keyed on arrival time.
type pqItem struct {
	node    string
	arrival time.Time
}

type pqueue []pqItem

func (q pqueue) Len() int            { return len(q) }
func (q pqueue) Less(i, j int) bool  { return q[i].arrival.Before(q[j].arrival) }
func (q pqueue) Swap(i, j int)       { q[i], q[j] = q[j], q[i] }
func (q *pqueue) Push(x interface{}) { *q = append(*q, x.(pqItem)) }
func (q *pqueue) Pop() interface{} {
	old := *q
	n := len(old)
	item := old[n-1]
	*q = old[:n-1]
	return item
}
