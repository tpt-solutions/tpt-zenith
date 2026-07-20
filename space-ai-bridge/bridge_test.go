// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge_test

import (
	"testing"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/cgr"
	"github.com/TPT-Solutions/tpt-zenith/routing/sim"
	spaceaibridge "github.com/TPT-Solutions/tpt-zenith/space-ai-bridge"
)

// syntheticScene mirrors the model-side helper: a deterministic grayscale scene.
func syntheticScene(n int, brightFraction float64) []byte {
	scene := make([]byte, n)
	brightEvery := 0
	if brightFraction > 0 {
		brightEvery = int(1.0 / brightFraction)
	}
	for i := range scene {
		if brightEvery > 0 && i%brightEvery == 0 {
			scene[i] = 255
		} else {
			scene[i] = 100
		}
	}
	return scene
}

// TestOnOrbitInferenceOverDTN is the Phase 5 end-to-end demonstration: a model
// is deployed to a satellite compute node over DTN, an inference request is sent
// (which references a scene already on-orbit, never uplinking the raw data), and
// only the compact result is downlinked back to the ground station across a
// later pass. It verifies delivery and a large bandwidth saving.
func TestOnOrbitInferenceOverDTN(t *testing.T) {
	base := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
	at := func(min float64) time.Time {
		return base.Add(time.Duration(min * float64(time.Minute)))
	}

	const (
		groundNode = "dtn://ground-1"
		satNode    = "dtn://sat-eo"
	)

	// Contact plan: an early uplink pass for the model, a later uplink pass for
	// the request, and a still-later downlink pass for the result. The response
	// must therefore be stored on-orbit across a coverage gap.
	plan := cgr.NewContactPlan([]cgr.Contact{
		{From: groundNode, To: satNode, Start: at(2), End: at(8), DataRate: 1e6, Confidence: 1},
		{From: groundNode, To: satNode, Start: at(20), End: at(26), DataRate: 1e6, Confidence: 1},
		{From: satNode, To: groundNode, Start: at(50), End: at(56), DataRate: 1e6, Confidence: 1},
	})

	// The satellite has already captured a 1 MiB scene on-orbit.
	scene := syntheticScene(1<<20, 0.15)
	compute := spaceaibridge.NewComputeNode()
	compute.CaptureScene("scene-42", scene)

	mesh := sim.NewMesh(plan)
	mesh.SetHandler(satNode, spaceaibridge.Responder(compute, 24*time.Hour))

	client := spaceaibridge.NewClient(
		bundle.MustParseEID("dtn://ground-1/space-ai"),
		bundle.MustParseEID("dtn://sat-eo/space-ai"),
	)

	uploadB, err := client.UploadBundle(
		spaceaibridge.ModelUpload{ModelID: "eo-stats", Kind: "scene-stats", Params: []byte{200}},
		at(0),
	)
	if err != nil {
		t.Fatal(err)
	}
	reqB, err := client.RequestBundle(
		spaceaibridge.InferenceRequest{RequestID: "req-1", ModelID: "eo-stats", InputID: "scene-42"},
		at(10),
	)
	if err != nil {
		t.Fatal(err)
	}

	// The request that crosses the link is tiny compared to the raw scene.
	reqPayload := reqB.Payload()
	if len(reqPayload) >= len(scene)/1000 {
		t.Fatalf("request payload %d bytes unexpectedly large vs scene %d", len(reqPayload), len(scene))
	}

	mesh.Inject(groundNode, uploadB, at(0))
	mesh.Inject(groundNode, reqB, at(10))

	report := mesh.Run()

	// Find the inference response delivered back to the ground station.
	var got *bundle.Bundle
	for _, d := range report.Deliveries {
		if d.Node != groundNode {
			continue
		}
		b, err := bundle.Unmarshal(mustMarshal(t, d))
		if err != nil {
			continue
		}
		if _, err := spaceaibridge.ParseResponse(b); err == nil {
			got = b
		}
	}
	if got == nil {
		t.Fatalf("no inference response delivered to ground; events=%+v", report.Events)
	}

	resp, err := spaceaibridge.ParseResponse(got)
	if err != nil {
		t.Fatal(err)
	}
	if !resp.OK {
		t.Fatalf("inference failed: %s", resp.Summary)
	}
	if resp.InputBytes != uint64(len(scene)) {
		t.Fatalf("InputBytes = %d, want %d", resp.InputBytes, len(scene))
	}
	savings := resp.BandwidthSavings()
	if savings < 0.90 {
		t.Fatalf("bandwidth savings %.4f below 90%%", savings)
	}
	t.Logf("delivered result: %s", resp.Summary)
	t.Logf("raw scene %d bytes -> result %d bytes (%.3f%% bandwidth saved)",
		resp.InputBytes, resp.OutputBytes, savings*100)

	// The model must have been deployed on-orbit.
	if models := compute.DeployedModels(); len(models) != 1 || models[0] != "eo-stats" {
		t.Fatalf("deployed models = %v", models)
	}
}

// mustMarshal re-serializes a delivered payload back into a full bundle wrapper
// is unnecessary here; the delivery already carries the raw payload. This helper
// simply wraps the delivered payload into a minimal bundle so ParseResponse can
// operate on a *bundle.Bundle.
func mustMarshal(t *testing.T, d sim.Delivery) []byte {
	t.Helper()
	b := bundle.NewBundle(
		bundle.MustParseEID("dtn://sat-eo/space-ai"),
		bundle.MustParseEID("dtn://ground-1/space-ai"),
		d.Payload,
		bundle.Options{CreationTime: 1},
	)
	enc, err := b.Marshal()
	if err != nil {
		t.Fatal(err)
	}
	return enc
}
