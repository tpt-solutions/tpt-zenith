// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import "testing"

func TestComputeNodeHandleUploadThenInfer(t *testing.T) {
	node := NewComputeNode()
	scene := syntheticScene(50000, 0.3)
	node.CaptureScene("scene-1", scene)

	up, _ := Encode(ModelUpload{ModelID: "m1", Kind: "scene-stats", Params: []byte{200}})
	reply, hasReply, err := node.Handle(up)
	if err != nil {
		t.Fatalf("handle upload: %v", err)
	}
	if hasReply || reply != nil {
		t.Fatal("model upload should not produce a reply")
	}
	if got := node.DeployedModels(); len(got) != 1 || got[0] != "m1" {
		t.Fatalf("deployed models = %v", got)
	}

	req, _ := Encode(InferenceRequest{RequestID: "r1", ModelID: "m1", InputID: "scene-1"})
	reply, hasReply, err = node.Handle(req)
	if err != nil {
		t.Fatalf("handle request: %v", err)
	}
	if !hasReply {
		t.Fatal("inference request should produce a reply")
	}
	msg, err := Decode(reply)
	if err != nil {
		t.Fatal(err)
	}
	resp := msg.(InferenceResponse)
	if !resp.OK {
		t.Fatalf("inference not ok: %s", resp.Summary)
	}
	if resp.InputBytes != uint64(len(scene)) {
		t.Fatalf("InputBytes = %d, want %d", resp.InputBytes, len(scene))
	}
	if resp.OutputBytes == 0 || resp.OutputBytes >= resp.InputBytes {
		t.Fatalf("OutputBytes %d not smaller than input %d", resp.OutputBytes, resp.InputBytes)
	}
}

func TestComputeNodeMissingModelAndScene(t *testing.T) {
	node := NewComputeNode()

	resp := node.RunInference(InferenceRequest{RequestID: "r", ModelID: "absent", InputID: "none"})
	if resp.OK {
		t.Fatal("expected failure for missing model")
	}

	up := ModelUpload{ModelID: "m", Kind: "cloud-mask"}
	if err := node.Deploy(up); err != nil {
		t.Fatal(err)
	}
	resp = node.RunInference(InferenceRequest{RequestID: "r", ModelID: "m", InputID: "none"})
	if resp.OK {
		t.Fatal("expected failure for missing scene")
	}
}

func TestComputeNodeUnknownKind(t *testing.T) {
	node := NewComputeNode()
	if err := node.Deploy(ModelUpload{ModelID: "x", Kind: "nope"}); err == nil {
		t.Fatal("expected error for unknown model kind")
	}
}
