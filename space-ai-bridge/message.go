// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import (
	"fmt"

	"github.com/TPT-Solutions/tpt-zenith/routing/cbor"
)

// MessageType identifies a Space-AI Bridge application message. Messages are
// carried as the payload (application data unit) of a DTN bundle.
type MessageType uint64

const (
	// MsgModelUpload deploys a model onto a satellite compute node.
	MsgModelUpload MessageType = 1
	// MsgInferenceRequest asks a compute node to run a model on an on-orbit
	// scene and return only the result.
	MsgInferenceRequest MessageType = 2
	// MsgInferenceResponse carries an inference result back to the requester.
	MsgInferenceResponse MessageType = 3
)

// Message is a Space-AI Bridge application message.
type Message interface {
	messageType() MessageType
	fields() []interface{}
}

// ModelUpload deploys a model of a given Kind (a built-in inference routine
// available on the compute node) under a caller-chosen ModelID, parameterized
// by opaque Params. Deploying a model on-orbit is what enables results, rather
// than raw data, to be returned to Earth.
type ModelUpload struct {
	ModelID string
	Kind    string
	Params  []byte
}

func (ModelUpload) messageType() MessageType { return MsgModelUpload }
func (m ModelUpload) fields() []interface{} {
	return []interface{}{m.ModelID, m.Kind, m.Params}
}

// InferenceRequest asks the compute node to run ModelID over the on-orbit scene
// identified by InputID. Crucially it does not carry the raw scene: the data
// already resides on the satellite (captured by its sensors), so only a small
// request travels over the link.
type InferenceRequest struct {
	RequestID string
	ModelID   string
	InputID   string
}

func (InferenceRequest) messageType() MessageType { return MsgInferenceRequest }
func (r InferenceRequest) fields() []interface{} {
	return []interface{}{r.RequestID, r.ModelID, r.InputID}
}

// InferenceResponse carries the compact result of an on-orbit inference, along
// with the byte accounting used to demonstrate bandwidth savings: InputBytes is
// the size of the raw scene processed on-orbit (and never transmitted), while
// OutputBytes is the size of Result actually returned to Earth.
type InferenceResponse struct {
	RequestID   string
	ModelID     string
	OK          bool
	Result      []byte
	Summary     string
	InputBytes  uint64
	OutputBytes uint64
}

func (InferenceResponse) messageType() MessageType { return MsgInferenceResponse }
func (r InferenceResponse) fields() []interface{} {
	return []interface{}{
		r.RequestID, r.ModelID, r.OK, r.Result, r.Summary, r.InputBytes, r.OutputBytes,
	}
}

// BandwidthSavings returns the fraction of bytes avoided by returning the
// processed result instead of downlinking the raw scene, in [0, 1]. It is zero
// if the input size is unknown.
func (r InferenceResponse) BandwidthSavings() float64 {
	if r.InputBytes == 0 {
		return 0
	}
	saved := 1.0 - float64(r.OutputBytes)/float64(r.InputBytes)
	if saved < 0 {
		return 0
	}
	return saved
}

// Encode serializes a message to bytes suitable for a bundle payload. The wire
// form is a CBOR array whose first element is the message type.
func Encode(msg Message) ([]byte, error) {
	arr := make([]interface{}, 0, 8)
	arr = append(arr, uint64(msg.messageType()))
	arr = append(arr, msg.fields()...)
	return cbor.Marshal(arr)
}

// Decode parses a message previously produced by Encode.
func Decode(data []byte) (Message, error) {
	v, _, err := cbor.Unmarshal(data)
	if err != nil {
		return nil, fmt.Errorf("spaceaibridge: decode: %w", err)
	}
	arr, ok := v.([]interface{})
	if !ok || len(arr) == 0 {
		return nil, fmt.Errorf("spaceaibridge: message must be a non-empty array")
	}
	mt, err := asUint64(arr[0])
	if err != nil {
		return nil, fmt.Errorf("spaceaibridge: message type: %w", err)
	}
	body := arr[1:]
	switch MessageType(mt) {
	case MsgModelUpload:
		if len(body) != 3 {
			return nil, fmt.Errorf("spaceaibridge: model upload needs 3 fields, got %d", len(body))
		}
		id, err := asString(body[0])
		if err != nil {
			return nil, err
		}
		kind, err := asString(body[1])
		if err != nil {
			return nil, err
		}
		params, err := asBytes(body[2])
		if err != nil {
			return nil, err
		}
		return ModelUpload{ModelID: id, Kind: kind, Params: params}, nil
	case MsgInferenceRequest:
		if len(body) != 3 {
			return nil, fmt.Errorf("spaceaibridge: inference request needs 3 fields, got %d", len(body))
		}
		reqID, err := asString(body[0])
		if err != nil {
			return nil, err
		}
		modelID, err := asString(body[1])
		if err != nil {
			return nil, err
		}
		inputID, err := asString(body[2])
		if err != nil {
			return nil, err
		}
		return InferenceRequest{RequestID: reqID, ModelID: modelID, InputID: inputID}, nil
	case MsgInferenceResponse:
		if len(body) != 7 {
			return nil, fmt.Errorf("spaceaibridge: inference response needs 7 fields, got %d", len(body))
		}
		reqID, err := asString(body[0])
		if err != nil {
			return nil, err
		}
		modelID, err := asString(body[1])
		if err != nil {
			return nil, err
		}
		okv, err := asBool(body[2])
		if err != nil {
			return nil, err
		}
		result, err := asBytes(body[3])
		if err != nil {
			return nil, err
		}
		summary, err := asString(body[4])
		if err != nil {
			return nil, err
		}
		inBytes, err := asUint64(body[5])
		if err != nil {
			return nil, err
		}
		outBytes, err := asUint64(body[6])
		if err != nil {
			return nil, err
		}
		return InferenceResponse{
			RequestID:   reqID,
			ModelID:     modelID,
			OK:          okv,
			Result:      result,
			Summary:     summary,
			InputBytes:  inBytes,
			OutputBytes: outBytes,
		}, nil
	default:
		return nil, fmt.Errorf("spaceaibridge: unknown message type %d", mt)
	}
}

func asUint64(v interface{}) (uint64, error) {
	switch x := v.(type) {
	case uint64:
		return x, nil
	case int64:
		if x < 0 {
			return 0, fmt.Errorf("spaceaibridge: negative value %d where unsigned expected", x)
		}
		return uint64(x), nil
	default:
		return 0, fmt.Errorf("spaceaibridge: expected unsigned integer, got %T", v)
	}
}

func asString(v interface{}) (string, error) {
	s, ok := v.(string)
	if !ok {
		return "", fmt.Errorf("spaceaibridge: expected text string, got %T", v)
	}
	return s, nil
}

func asBytes(v interface{}) ([]byte, error) {
	b, ok := v.([]byte)
	if !ok {
		return nil, fmt.Errorf("spaceaibridge: expected byte string, got %T", v)
	}
	return b, nil
}

func asBool(v interface{}) (bool, error) {
	b, ok := v.(bool)
	if !ok {
		return false, fmt.Errorf("spaceaibridge: expected boolean, got %T", v)
	}
	return b, nil
}
