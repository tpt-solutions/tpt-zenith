// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import (
	"fmt"
	"sync"
)

// ComputeNode is a simulated satellite compute node. It hosts deployed models,
// holds raw scenes captured on-orbit by its sensors, and runs inference
// requests, returning only the compact results. Raw scenes are never
// transmitted, which is where the bandwidth savings come from.
//
// ComputeNode is safe for concurrent use.
type ComputeNode struct {
	mu        sync.Mutex
	factories map[string]ModelFactory
	models    map[string]Model
	scenes    map[string][]byte
}

// NewComputeNode creates a compute node with the built-in model kinds
// registered.
func NewComputeNode() *ComputeNode {
	return &ComputeNode{
		factories: BuiltinFactories(),
		models:    map[string]Model{},
		scenes:    map[string][]byte{},
	}
}

// RegisterFactory adds or overrides a model kind the node can instantiate.
func (c *ComputeNode) RegisterFactory(kind string, f ModelFactory) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.factories[kind] = f
}

// CaptureScene stores a raw scene under inputID, simulating an on-orbit sensor
// capture. The scene stays on the satellite; only inference results leave it.
func (c *ComputeNode) CaptureScene(inputID string, data []byte) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.scenes[inputID] = append([]byte(nil), data...)
}

// Deploy instantiates and stores a model from a ModelUpload message.
func (c *ComputeNode) Deploy(up ModelUpload) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	factory, ok := c.factories[up.Kind]
	if !ok {
		return fmt.Errorf("spaceaibridge: unknown model kind %q", up.Kind)
	}
	model, err := factory(up.ModelID, up.Params)
	if err != nil {
		return fmt.Errorf("spaceaibridge: deploy %q: %w", up.ModelID, err)
	}
	c.models[up.ModelID] = model
	return nil
}

// RunInference executes a request against a deployed model and on-orbit scene,
// returning the response (including byte accounting for bandwidth savings).
func (c *ComputeNode) RunInference(req InferenceRequest) InferenceResponse {
	c.mu.Lock()
	model, hasModel := c.models[req.ModelID]
	scene, hasScene := c.scenes[req.InputID]
	c.mu.Unlock()

	resp := InferenceResponse{RequestID: req.RequestID, ModelID: req.ModelID}
	if !hasModel {
		resp.Summary = fmt.Sprintf("model %q not deployed", req.ModelID)
		return resp
	}
	if !hasScene {
		resp.Summary = fmt.Sprintf("scene %q not available on-orbit", req.InputID)
		return resp
	}
	result, summary, err := model.Infer(scene)
	if err != nil {
		resp.Summary = fmt.Sprintf("inference failed: %v", err)
		return resp
	}
	resp.OK = true
	resp.Result = result
	resp.Summary = summary
	resp.InputBytes = uint64(len(scene))
	resp.OutputBytes = uint64(len(result))
	return resp
}

// Handle processes an inbound application message payload. It returns an
// optional reply payload (present only for inference requests) and an error for
// malformed input. Model uploads are applied and produce no reply.
func (c *ComputeNode) Handle(payload []byte) (reply []byte, hasReply bool, err error) {
	msg, err := Decode(payload)
	if err != nil {
		return nil, false, err
	}
	switch m := msg.(type) {
	case ModelUpload:
		if err := c.Deploy(m); err != nil {
			return nil, false, err
		}
		return nil, false, nil
	case InferenceRequest:
		resp := c.RunInference(m)
		enc, err := Encode(resp)
		if err != nil {
			return nil, false, err
		}
		return enc, true, nil
	default:
		return nil, false, fmt.Errorf("spaceaibridge: compute node cannot handle message type %d", msg.messageType())
	}
}

// DeployedModels returns the IDs of models currently deployed on the node.
func (c *ComputeNode) DeployedModels() []string {
	c.mu.Lock()
	defer c.mu.Unlock()
	out := make([]string, 0, len(c.models))
	for id := range c.models {
		out = append(out, id)
	}
	return out
}
