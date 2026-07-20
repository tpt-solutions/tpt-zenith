// SPDX-FileCopyrightText: 2026 TPT Solutions
//
// SPDX-License-Identifier: MIT OR Apache-2.0

package spaceaibridge

import (
	"strings"
	"testing"
)

// syntheticScene builds a deterministic grayscale "image" of n pixels with a
// given fraction of bright (255) pixels, the rest mid-gray (100).
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

func TestSceneStatsCompactAndDeterministic(t *testing.T) {
	m, err := newSceneStats("stats-1", []byte{200})
	if err != nil {
		t.Fatal(err)
	}
	scene := syntheticScene(100000, 0.2)
	r1, s1, err := m.Infer(scene)
	if err != nil {
		t.Fatalf("Infer: %v", err)
	}
	if len(r1) == 0 || len(r1) >= len(scene) {
		t.Fatalf("result size %d not compact vs scene %d", len(r1), len(scene))
	}
	if len(r1) > 128 {
		t.Fatalf("result %d bytes larger than expected", len(r1))
	}
	// Deterministic.
	r2, s2, _ := m.Infer(scene)
	if string(r1) != string(r2) || s1 != s2 {
		t.Fatal("scene-stats not deterministic")
	}
}

func TestCloudMaskClassifies(t *testing.T) {
	m, err := newCloudMask("cloud-1", nil)
	if err != nil {
		t.Fatal(err)
	}
	clear := syntheticScene(10000, 0.02)
	_, summary, err := m.Infer(clear)
	if err != nil {
		t.Fatal(err)
	}
	if want := "class=clear"; !strings.Contains(summary, want) {
		t.Fatalf("summary %q missing %q", summary, want)
	}

	overcast := syntheticScene(10000, 0.9)
	_, summary, _ = m.Infer(overcast)
	if want := "class=overcast"; !strings.Contains(summary, want) {
		t.Fatalf("summary %q missing %q", summary, want)
	}
}

func TestModelEmptySceneErrors(t *testing.T) {
	m, _ := newSceneStats("s", nil)
	if _, _, err := m.Infer(nil); err == nil {
		t.Fatal("expected error on empty scene")
	}
}
