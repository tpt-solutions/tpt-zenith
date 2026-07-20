// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import (
	"fmt"
	"sync/atomic"
	"time"

	"github.com/TPT-Solutions/tpt-zenith/routing/bundle"
	"github.com/TPT-Solutions/tpt-zenith/routing/sim"
)

// DefaultLifetime is the bundle lifetime used when a client does not specify
// one. On-orbit round trips can span multiple passes, so it is generous.
const DefaultLifetime = 24 * time.Hour

// Client is the ground-station side of the Space-AI Bridge API. It builds the
// DTN bundles that deploy models and request inference, addressed to a
// satellite compute node, and parses the responses that come back.
type Client struct {
	// Source is this ground station's endpoint (also the reply destination).
	Source bundle.EndpointID
	// Satellite is the compute node's endpoint.
	Satellite bundle.EndpointID
	// Lifetime overrides DefaultLifetime when non-zero.
	Lifetime time.Duration

	seq atomic.Uint64
}

// NewClient creates a client addressing satellite from source.
func NewClient(source, satellite bundle.EndpointID) *Client {
	return &Client{Source: source, Satellite: satellite}
}

func (c *Client) lifetime() time.Duration {
	if c.Lifetime > 0 {
		return c.Lifetime
	}
	return DefaultLifetime
}

func (c *Client) options(createdAt time.Time) bundle.Options {
	return bundle.Options{
		Lifetime:       c.lifetime(),
		CreationTime:   bundle.DTNTime(createdAt),
		SequenceNumber: c.seq.Add(1),
	}
}

// UploadBundle builds a bundle that deploys a model on the compute node.
func (c *Client) UploadBundle(up ModelUpload, createdAt time.Time) (*bundle.Bundle, error) {
	payload, err := Encode(up)
	if err != nil {
		return nil, err
	}
	return bundle.NewBundle(c.Source, c.Satellite, payload, c.options(createdAt)), nil
}

// RequestBundle builds a bundle that requests on-orbit inference.
func (c *Client) RequestBundle(req InferenceRequest, createdAt time.Time) (*bundle.Bundle, error) {
	payload, err := Encode(req)
	if err != nil {
		return nil, err
	}
	return bundle.NewBundle(c.Source, c.Satellite, payload, c.options(createdAt)), nil
}

// ParseResponse decodes an inference response from a delivered bundle.
func ParseResponse(b *bundle.Bundle) (InferenceResponse, error) {
	return ParseResponsePayload(b.Payload())
}

// ParseResponsePayload decodes an inference response from a raw bundle payload,
// convenient when only the delivered application data unit is available.
func ParseResponsePayload(payload []byte) (InferenceResponse, error) {
	msg, err := Decode(payload)
	if err != nil {
		return InferenceResponse{}, err
	}
	resp, ok := msg.(InferenceResponse)
	if !ok {
		return InferenceResponse{}, fmt.Errorf("spaceaibridge: expected an inference response, got type %d", msg.messageType())
	}
	return resp, nil
}

// Responder adapts a ComputeNode into a sim.AppHandler so it can serve requests
// delivered over the DTN mesh, emitting result bundles back toward each
// requester. Model uploads are applied silently (no reply bundle). The returned
// handler is bound to node and reuses lifetime for reply bundles.
func Responder(node *ComputeNode, lifetime time.Duration) sim.AppHandler {
	if lifetime <= 0 {
		lifetime = DefaultLifetime
	}
	var replySeq atomic.Uint64
	return func(deliveredTo string, b *bundle.Bundle, now time.Time) []sim.OutboundBundle {
		reply, hasReply, err := node.Handle(b.Payload())
		if err != nil || !hasReply {
			return nil
		}
		// The reply travels from the compute node (this bundle's destination)
		// back to the original requester (this bundle's source).
		rb := bundle.NewBundle(b.Primary.Destination, b.Primary.Source, reply, bundle.Options{
			Lifetime:       lifetime,
			CreationTime:   bundle.DTNTime(now),
			SequenceNumber: replySeq.Add(1),
		})
		return []sim.OutboundBundle{{FromNode: deliveredTo, Bundle: rb, At: now}}
	}
}
