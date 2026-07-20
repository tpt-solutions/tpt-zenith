# TPT Zenith — Project Todo

Open-source ground station control & orbital routing platform. Pre-alpha, dual-licensed MIT/Apache-2.0, TPT Solutions.

Software-only simulation first; hardware integration deferred to Phase 7.

---

## Phase 0 — Repo & Foundation
Goal: scaffold the monorepo, licensing, and CI so later phases have somewhere to land.

- [x] Monorepo layout: Rust workspace (`antenna-control/`, `orbital-mechanics/`) + Go module (`routing/`, `space-ai-bridge/`)
- [x] Dual license setup: `LICENSE-MIT`, `LICENSE-APACHE`, SPDX headers on source files, TPT Solutions copyright
- [x] README: project overview, architecture summary, contributing guide stub
- [x] Basic CI: build + test for Rust workspace and Go module
- [x] `.gitignore`, `.editorconfig`, issue/PR templates

## Phase 1 — Orbital Mechanics Engine (Rust, simulation-only)
Goal: predict satellite positions and visibility windows without any live data feed.

- [ ] Custom SGP4/SDP4 propagator crate (TLE parsing, position/velocity at epoch)
  - [x] Near-Earth SGP4 path (accurate to reference `tcppver.out` vectors)
  - [x] Fix `dsinit`: irez==1 (24h resonance) `del1`/`del2`/`del3` were stored into the wrong struct fields (`d2201`/`d2211`, and `del3` was dropped entirely) instead of the dedicated `del1`/`del2`/`del3` fields
  - [x] Fix `dspace`: the simple-vs-full resonance formula branch was gated on `irez != 1` instead of `irez != 2` — 24h- and 12h-resonance satellites were swapped onto each other's formulas
  - [x] Remove dead/incorrect scaffold code in `dsinit` (unused `em_`/`inclm_`/`argpm_`/`nodem_`/`mm_` locals) and debug scaffolding (`ZENITH_DEBUG` eprintln, `examples/reference_check.rs`)
  - [ ] Re-verify full 33-satellite `tests/verification.rs` suite against the corrected `dspace`/`dsinit` and shrink the `excluded` list to only genuine remaining edge cases
  - [ ] Investigate satellite 22312 (near-Earth, `isimp==1` high-drag path) — separate bug, unrelated to the deep-space fixes above
  - [ ] Investigate satellite 23333 (e≈0.973, very low mean motion) — large residual even after the resonance-branch fix; needs its own diagnosis
- [ ] Visibility window calculation (look angles, AOS/LOS for a ground station)
- [ ] Handoff optimization logic (select next satellite as current one sets)
- [ ] Unit tests against known TLE/SGP4 reference vectors
- [ ] Simulated constellation generator (synthetic Starlink/OneWeb-like TLE sets)

## Phase 2 — Orbital Routing Protocol (Go, DTN7/Bundle Protocol RFC 9171)
Goal: route data through a simulated intermittent satellite mesh using contact schedules from Phase 1.

- [ ] Bundle Protocol primitives (bundle format, endpoint IDs, creation timestamps)
- [ ] Store-and-forward node with contact-graph routing
- [ ] Simulated multi-node mesh (satellite + ground station nodes)
- [ ] CLI/test harness: send a bundle through simulated intermittent links, confirm delivery
- [ ] Integration test: Phase 1 visibility windows feed routing contact schedule end-to-end

## Phase 3 — Antenna Control System (Rust, simulated hardware)
Goal: deterministic tracking loop against a simulated dish, ready for real hardware later.

- [ ] Deterministic tracking loop (orbital mechanics output → pointing commands)
- [ ] Hardware abstraction layer with a simulated dish backend
- [ ] Sub-degree tracking accuracy tests against simulated satellite passes
- [ ] Interface contract defined for future real microcontroller backend

## Phase 4 — RF Signal Processing Pipeline (SDR, simulation-first)
Goal: define and simulate the modem/error-correction chain before touching real SDR hardware.

- [ ] Modulation/demodulation + error correction interfaces
- [ ] Software simulation of the signal chain (no real SDR required)
- [ ] Spike/decision doc: GNU Radio integration vs custom Rust SDR stack
- [ ] CCSDS standard compliance notes for framing/coding

## Phase 5 — Space-AI Bridge (on-orbit inference API)
Goal: demonstrate on-orbit inference and bandwidth savings, over the routing layer, in simulation.

- [ ] API layer for model upload + inference request/response over DTN routing
- [ ] Simulated "satellite compute node" running a mock/lightweight model
- [ ] Bandwidth savings demonstration (raw vs processed payload size comparison)

## Phase 6 — TPT Ecosystem Integration
Goal: wire Zenith into the rest of the TPT stack.

- [ ] Zenith → TPT Aether: data handoff interface (routing output to terrestrial basestation)
- [ ] Zenith → TPT DataCenter: processed data storage interface
- [ ] Zenith → TPT Sentinel: secure comms interface notes (C4ISR use case, security requirements)

## Phase 7 — Hardware-in-the-Loop (future / post-simulation)
Goal: replace simulated backends with real hardware.

- [ ] Real SDR hardware integration (HackRF/USRP)
- [ ] Real dish antenna microcontroller integration (replaces Phase 3 simulated backend)
- [ ] Field test plan against a real satellite pass (e.g. NOAA APT or amateur CubeSat)
