// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// Command zenith-dtn is a test harness for the Orbital Routing Protocol. It
// sends a bundle through a simulated intermittent satellite mesh and reports
// whether it was delivered, printing the full store-and-forward trace.
//
// With no -plan flag it runs a built-in demonstration in which a bundle created
// at a ground station is uplinked to a satellite, carried on-orbit across a
// coverage gap, and downlinked to a second ground station. With -plan it loads
// a contact plan exported from the Phase 1 orbital-mechanics engine.
//
// Examples:
//
//	zenith-dtn
//	zenith-dtn -plan contacts.json -src dtn://ground-tokyo/out \
//	    -dst dtn://ground-kauai/inbox -payload "hello"
package main

import (
	"flag"
	"fmt"
	"os"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
	"github.com/TPT-Solutions/tpt-zenith/routing/contactplan"
	"github.com/TPT-Solutions/tpt-zenith/routing/sim"
)

func main() {
	planPath := flag.String("plan", "", "path to a contact plan JSON exported by the orbital-mechanics engine")
	src := flag.String("src", "dtn://ground-1/out", "source endpoint ID")
	dst := flag.String("dst", "dtn://ground-2/inbox", "destination endpoint ID")
	payload := flag.String("payload", "orbital edge payload", "payload text to carry")
	injectMin := flag.Float64("inject-min", 0, "minutes after the plan epoch at which to inject the bundle")
	lifetime := flag.Duration("lifetime", 24*time.Hour, "bundle lifetime")
	flag.Parse()

	if err := run(*planPath, *src, *dst, *payload, *injectMin, *lifetime); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

func run(planPath, src, dst, payload string, injectMin float64, lifetime time.Duration) error {
	sourceEID, err := bundle.ParseEID(src)
	if err != nil {
		return err
	}
	destEID, err := bundle.ParseEID(dst)
	if err != nil {
		return err
	}

	var (
		sched *contactplan.Schedule
	)
	if planPath != "" {
		sched, err = contactplan.LoadFile(planPath)
		if err != nil {
			return err
		}
		fmt.Printf("loaded contact plan: epoch %s, %d contacts\n",
			sched.Epoch.Format(time.RFC3339), len(sched.Plan.Contacts()))
	} else {
		sched = demoSchedule()
		fmt.Println("no -plan given; using built-in demo contact plan")
	}

	injectAt := sched.At(injectMin)
	b := bundle.NewBundle(sourceEID, destEID, []byte(payload), bundle.Options{
		Lifetime:     lifetime,
		CreationTime: bundle.DTNTime(injectAt),
	})

	m := sim.NewMesh(sched.Plan)
	m.Inject(sourceEID.NodeID(), b, injectAt)
	report := m.Run()

	fmt.Printf("\nbundle %s\n  %s -> %s\n  payload: %q (%d bytes)\n\n",
		b.ID(), sourceEID, destEID, payload, len(payload))

	fmt.Println("trace:")
	for _, e := range report.Events {
		rel := e.Time.Sub(sched.Epoch)
		line := fmt.Sprintf("  t+%-10s %-13s %s", rel.Round(time.Second), e.Kind, e.Node)
		if e.NextHop != "" {
			line += " -> " + e.NextHop
		}
		fmt.Println(line)
	}

	if !report.DeliveredID(b.ID()) {
		return fmt.Errorf("bundle was NOT delivered")
	}
	for _, d := range report.Deliveries {
		if d.BundleID == b.ID() {
			fmt.Printf("\nDELIVERED to %s at t+%s (delay-tolerant transit of %s)\n",
				d.Node, d.Time.Sub(sched.Epoch).Round(time.Second),
				d.Time.Sub(injectAt).Round(time.Second))
		}
	}
	return nil
}

// demoSchedule builds a self-contained store-and-forward scenario: a ground
// station uplinks to a satellite, which later downlinks to a second ground
// station it could not reach at the same time.
func demoSchedule() *contactplan.Schedule {
	epoch := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	m := func(minutes float64) time.Time {
		return epoch.Add(time.Duration(minutes * float64(time.Minute)))
	}
	plan := cgr.NewContactPlan([]cgr.Contact{
		{From: "dtn://ground-1", To: "dtn://sat-1", Start: m(2), End: m(8), DataRate: 125000, OWLT: 2 * time.Millisecond, Confidence: 1},
		{From: "dtn://sat-1", To: "dtn://ground-1", Start: m(2), End: m(8), DataRate: 125000, OWLT: 2 * time.Millisecond, Confidence: 1},
		{From: "dtn://sat-1", To: "dtn://ground-2", Start: m(52), End: m(58), DataRate: 125000, OWLT: 3 * time.Millisecond, Confidence: 1},
		{From: "dtn://ground-2", To: "dtn://sat-1", Start: m(52), End: m(58), DataRate: 125000, OWLT: 3 * time.Millisecond, Confidence: 1},
	})
	return &contactplan.Schedule{Epoch: epoch, Plan: plan}
}
