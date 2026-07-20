# TPT Zenith

Open-source ground station control & orbital routing platform.

**Status:** Pre-alpha · **License:** Dual MIT / Apache-2.0 · **Owner:** TPT Solutions

TPT Zenith is an open-source ground station control system and orbital routing
protocol. It manages dish arrays, tracks satellite constellations, and routes
data between space-based AI inference nodes and terrestrial TPT Aether
basestations. Software-only simulation first; hardware integration is deferred
to Phase 7.

## Architecture

| Component | Language | Phase | Description |
|-----------|----------|-------|-------------|
| Orbital Mechanics Engine | Rust | 1 | SGP4/SDP4 propagator, visibility windows, handoff optimization |
| Orbital Routing Protocol | Go | 2 | DTN / Bundle Protocol (RFC 9171) store-and-forward mesh routing |
| Antenna Control System | Rust | 3 | Deterministic tracking loop with a simulated dish backend |
| RF Signal Processing Pipeline | SDR | 4 | Modem / error-correction chain (simulation-first) |
| Space-AI Bridge | Go | 5 | On-orbit inference API over the DTN routing layer |
| TPT Ecosystem Integration | — | 6 | Handoff interfaces to Aether, DataCenter, Sentinel |

The repository is a mixed Rust workspace and Go module:

```
tpt-zenith/
├── Cargo.toml              # Rust workspace
├── go.mod                  # Go module (github.com/TPT-Solutions/tpt-zenith)
├── antenna-control/        # Rust crate (Phase 3)
├── orbital-mechanics/      # Rust crate (Phase 1)
├── routing/                # Go package (Phase 2)
├── space-ai-bridge/        # Go package (Phase 5)
├── LICENSE-MIT
├── LICENSE-APACHE
└── todo.md                 # Phased roadmap
```

## Roadmap

Phased plan lives in [`todo.md`](./todo.md):

- **Phase 0** — Repo, licensing, CI (this phase)
- **Phase 1** — Orbital mechanics engine (Rust)
- **Phase 2** — Orbital routing protocol (Go, DTN)
- **Phase 3** — Antenna control system (Rust, simulated)
- **Phase 4** — RF signal processing pipeline (SDR, simulation-first)
- **Phase 5** — Space-AI bridge (on-orbit inference)
- **Phase 6** — TPT ecosystem integration
- **Phase 7** — Hardware-in-the-loop (future)

## Building

### Rust workspace

```sh
cargo build
cargo test
```

### Go module

```sh
go build ./...
go test ./...
```

### Orbital routing demo (Phase 2)

Run the DTN store-and-forward harness with its built-in intermittent-mesh
scenario:

```sh
go run ./routing/cmd/zenith-dtn
```

Or drive it with a contact plan exported from the Phase 1 orbital-mechanics
engine (visibility windows → DTN contacts), demonstrating the end-to-end
Phase 1 → Phase 2 data flow:

```sh
cargo run -p orbital-mechanics --bin export_contacts > contacts.json
go run ./routing/cmd/zenith-dtn -plan contacts.json \
    -src dtn://ground-tokyo/out -dst dtn://ground-kauai/inbox -payload "hello"
```

## Contributing

Contributions are welcome. Please read the issue and pull request templates and
ensure `cargo fmt` / `go fmt` are applied. All source files carry SPDX headers
and are dual-licensed MIT OR Apache-2.0. By contributing, you agree your
contributions are licensed under the same terms. (Full contributing guide to
follow.)
