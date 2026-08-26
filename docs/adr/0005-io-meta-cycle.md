# ADR-0005: Resolving the io ↔ meta dependency cycle

## Context

Slice-1 planning flagged a possible cycle: `nlp_parser` was suspected to
use `DetectedLanguage` from meta, while meta's chat handling was
suspected to use io. The workspace split forced the question: which
direction is real?

## Decision (from the actual code)

**There is no cycle — and there never was an edge in either direction
at library level.**

- `DetectedLanguage`, `NlpParser`, `ParseIntent` are defined and owned
  by **`omiai_io`** (`src/nlp_parser.rs`). Meta does not use them.
- `omiai-meta` depends on core, evolution, knowledge, memory,
  checkpoint, neuro — **not on io**.
- `omiai-io`'s Cargo.toml lists `omiai-meta` as a regular dependency,
  but the only actual usage in the whole crate tree is one
  **integration test** (`crates/omiai-io/tests/integration.rs`) pulling
  `omiai_meta::self_improvement::MetaCognitiveEngine`.

Resolution applied in slice 1: keep `DetectedLanguage` in io (it is an
io concern — input language detection), and treat the io→meta edge as
test-only. If a future slice needs `MetaCognitiveEngine` inside io's
library code, the engine's *interface* (not implementation) should be
moved down to a lower crate instead of growing the edge.

## Consequences

- Layering stays acyclic: io and meta are siblings over core/memory.
- `omiai-io`'s regular dependency on `omiai-meta` should shrink to a
  dev-dependency when its Cargo.toml is next touched.
