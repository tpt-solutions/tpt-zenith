# Spike: GNU Radio integration vs. a custom Rust SDR stack

**Status:** decided for Phase 4 (simulation-first). Revisit at the start of
Phase 7 (real SDR hardware).

## Question

`spec.txt` lists two options for the RF signal processing pipeline: wrap GNU
Radio, or build a custom Rust SDR stack. Which should Phase 4 build on?

## Decision

**Custom Rust stack**, implemented in this crate (`rf-pipeline`), with no
GNU Radio dependency for Phase 4. Revisit GNU Radio (or a Rust SDR hardware
crate such as `soapysdr`) specifically when Phase 7 adds a real HackRF/USRP
backend.

## Why

- **Phase 4 is explicitly simulation-first.** There's no SDR hardware, no
  sample-rate/clocking constraints, and no real-time deadline to hit. GNU
  Radio's value proposition — a mature scheduler, a huge library of hardware
  drivers and DSP blocks, GRC flowgraphs — is aimed at problems this phase
  doesn't have yet.
- **Consistency with the rest of the workspace.** `orbital-mechanics` and
  `antenna-control` are dependency-free, hand-rolled Rust; this crate follows
  the same pattern (see `complex.rs`'s minimal `Complex` type and
  `channel.rs`'s hand-rolled PRNG instead of pulling in `num-complex` or
  `rand`). Pulling in GNU Radio (a C++ framework with a Python flowgraph
  layer and its own build/runtime story) would be the first non-Rust,
  non-Go dependency in the entire monorepo, for no simulation-time benefit.
- **Interface clarity.** `Modulator`/`Demodulator` and `Encoder`/`Decoder` are
  small, testable Rust traits (see `modem.rs`, `fec.rs`) that plug directly
  into `simulate_link` and into the deterministic, seeded `AwgnChannel`
  (`channel.rs`). That gives reproducible, cheap-to-run CI tests (bit-exact
  round trips, BER comparisons) without a GNU Radio runtime in the test
  environment.
- **Cost of being wrong is low and bounded.** Nothing here is a dead end:
  the `Modulator`/`Demodulator`/`Encoder`/`Decoder` traits are the interface
  contract a GNU-Radio-backed or hardware-backed implementation would also
  need to satisfy. Swapping the implementation later doesn't require
  redesigning the pipeline.

## When to revisit

At the start of Phase 7 (hardware-in-the-loop), when a HackRF/USRP backend
is added:

- If the goal is fast access to a large existing library of hardware
  drivers and DSP blocks (matched filtering, real-time scheduling across
  multiple sample streams), GNU Radio becomes attractive again — likely as
  an optional, hardware-only integration path rather than a replacement for
  this crate's simulation path.
- If the goal is to keep the whole stack in Rust (easier to reason about
  across the antenna-control / orbital-mechanics / rf-pipeline boundary,
  single toolchain, no FFI), a Rust SDR hardware crate (e.g. `soapysdr`
  bindings) paired with this crate's existing modem/FEC/framing code is the
  lower-friction path.

Either way, the `Modulator`/`Demodulator`/`Encoder`/`Decoder` traits defined
here should still be the interface the hardware backend is validated
against, the same way `antenna-control`'s `DishBackend` trait lets a real
microcontroller backend replace `SimulatedDish` without touching the
tracking loop.
