// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import (
	"fmt"
	"math"

	"github.com/TPT-Solutions/tpt-zenith/routing/cbor"
)

// Model is a lightweight inference routine that runs on a satellite compute
// node. Implementations process a raw on-orbit scene (for example a captured
// image, passed as bytes) and return a compact result plus a human-readable
// summary. The whole point is that Result is far smaller than the input, so
// only the result need be downlinked.
type Model interface {
	// ID returns the deployed model identifier.
	ID() string
	// Kind returns the model kind (which built-in routine it is).
	Kind() string
	// Infer runs the model over a raw scene, returning a compact result and a
	// short summary.
	Infer(scene []byte) (result []byte, summary string, err error)
}

// ModelFactory constructs a model of a particular kind from opaque parameters,
// under the given deployed ID.
type ModelFactory func(id string, params []byte) (Model, error)

// BuiltinFactories returns the model kinds a compute node knows how to run.
// A ModelUpload names one of these kinds; the compute node instantiates it with
// the uploaded parameters.
func BuiltinFactories() map[string]ModelFactory {
	return map[string]ModelFactory{
		"scene-stats": newSceneStats,
		"cloud-mask":  newCloudMask,
	}
}

// sceneStats treats the scene as 8-bit grayscale pixels and returns compact
// summary statistics (count, min, max, mean, standard deviation) plus a count
// of "hot" pixels above a brightness threshold. A megapixel scene collapses to
// a few dozen bytes.
type sceneStats struct {
	id        string
	threshold uint8
}

func newSceneStats(id string, params []byte) (Model, error) {
	threshold := uint8(200)
	if len(params) >= 1 {
		threshold = params[0]
	}
	return &sceneStats{id: id, threshold: threshold}, nil
}

func (m *sceneStats) ID() string   { return m.id }
func (m *sceneStats) Kind() string { return "scene-stats" }

func (m *sceneStats) Infer(scene []byte) ([]byte, string, error) {
	if len(scene) == 0 {
		return nil, "", fmt.Errorf("spaceaibridge: empty scene")
	}
	var (
		sum  float64
		min  = uint8(255)
		max  = uint8(0)
		hot  uint64
		hist [16]uint64
	)
	for _, p := range scene {
		sum += float64(p)
		if p < min {
			min = p
		}
		if p > max {
			max = p
		}
		if p >= m.threshold {
			hot++
		}
		hist[p>>4]++
	}
	n := float64(len(scene))
	mean := sum / n
	var variance float64
	for _, p := range scene {
		d := float64(p) - mean
		variance += d * d
	}
	std := math.Sqrt(variance / n)

	histVals := make([]interface{}, len(hist))
	for i, h := range hist {
		histVals[i] = h
	}
	result, err := cbor.Marshal([]interface{}{
		uint64(len(scene)),
		uint64(min),
		uint64(max),
		mean,
		std,
		hot,
		histVals,
	})
	if err != nil {
		return nil, "", err
	}
	summary := fmt.Sprintf(
		"scene-stats: n=%d min=%d max=%d mean=%.1f std=%.1f hot(>=%d)=%d",
		len(scene), min, max, mean, std, m.threshold, hot,
	)
	return result, summary, nil
}

// cloudMask treats the scene as 8-bit grayscale pixels and estimates cloud
// cover as the fraction of bright pixels above a threshold, returning that
// fraction and a coverage class. This is the classic "don't downlink cloudy
// scenes" on-orbit filter.
type cloudMask struct {
	id        string
	threshold uint8
}

func newCloudMask(id string, params []byte) (Model, error) {
	threshold := uint8(190)
	if len(params) >= 1 {
		threshold = params[0]
	}
	return &cloudMask{id: id, threshold: threshold}, nil
}

func (m *cloudMask) ID() string   { return m.id }
func (m *cloudMask) Kind() string { return "cloud-mask" }

func (m *cloudMask) Infer(scene []byte) ([]byte, string, error) {
	if len(scene) == 0 {
		return nil, "", fmt.Errorf("spaceaibridge: empty scene")
	}
	var cloudy uint64
	for _, p := range scene {
		if p >= m.threshold {
			cloudy++
		}
	}
	fraction := float64(cloudy) / float64(len(scene))
	class := classify(fraction)
	result, err := cbor.Marshal([]interface{}{fraction, class})
	if err != nil {
		return nil, "", err
	}
	summary := fmt.Sprintf("cloud-mask: cover=%.1f%% class=%s", fraction*100, class)
	return result, summary, nil
}

func classify(fraction float64) string {
	switch {
	case fraction < 0.10:
		return "clear"
	case fraction < 0.50:
		return "partly-cloudy"
	default:
		return "overcast"
	}
}
