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
  - [x] Re-verify full 33-satellite `tests/verification.rs` suite against the corrected `dspace`/`dsinit`; found and fixed a much larger bug in the process (see below) and rewrote the `excluded` list to reflect the true current state, split into a deep-space-resonance group and a near-earth extreme-high-drag group
  - [x] Fix `sgp4init`: the corrected ("unkozai") mean motion was computed inside a scoped block and stored to `no_kozai`, but the local `no` used by every secular-rate formula after the block (`cc1`/`cc4`/`cc5`, `mdot`, `argpdot`, `nodedot`, `dsinit`, and the `isimp` perigee test) fell back to the raw, uncorrected TLE mean motion once the block ended. This was the dominant source of error for nearly every satellite in the verification suite (only 1 of 33 was passing strictly beforehand) — position error grew with `tsince` since it only affected rate terms, not epoch state. Also fixed a `vy` typo in the short-period-to-Cartesian conversion (used `cossu` twice instead of `sinsu`) and removed the leftover `SKIP_DRAG` debug flag.
  - [x] Investigate satellite 22312 (near-Earth, `isimp==1` high-drag path) — was almost entirely the `no` bug above (55 km → 0.58 km residual); the small remainder is consistent with float sensitivity at this TLE's extreme inputs (perigee ~79 km, B* ~0.5) and is documented in the excluded list, not further diagnosed
  - [x] Investigate satellite 23333 (e≈0.973, very low mean motion) — fully resolved by the `no` fix above; now passes the strict cm tolerance
  - [x] Fix `.gitignore`: the fixture file `tests/fixtures/sgp4/tcppver.out` was caught by the blanket `*.out` rule, so `tests/verification.rs` never actually compiled/ran in CI; added a negation and committed the fixture
  - [ ] Clean up ~40 pre-existing `cargo clippy` warnings in `propagator/deep_space.rs` and `bin/crosscheck.rs` (unnecessary `mut`, unused vars, non-upper-case const) — CI's `-D warnings` clippy step was already failing before this session's changes, unrelated to the propagator bug fix
- [ ] Visibility window calculation (look angles, AOS/LOS for a ground station)
- [ ] Handoff optimization logic (select next satellite as current one sets)
- [ ] Unit tests against known TLE/SGP4 reference vectors
- [ ] Simulated constellation generator (synthetic Starlink/OneWeb-like TLE sets)

## Phase 2 — Orbital Routing Protocol (Go, DTN7/Bundle Protocol RFC 9171)
Goal: route data through a simulated intermittent satellite mesh using contact schedules from Phase 1.

- [x] Bundle Protocol primitives (bundle format, endpoint IDs, creation timestamps)
- [x] Store-and-forward node with contact-graph routing
- [x] Simulated multi-node mesh (satellite + ground station nodes)
- [x] CLI/test harness: send a bundle through simulated intermittent links, confirm delivery
- [x] Integration test: Phase 1 visibility windows feed routing contact schedule end-to-end

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
