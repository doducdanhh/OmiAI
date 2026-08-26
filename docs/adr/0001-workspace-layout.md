# ADR-0001: Virtual Cargo workspace with 15 crates

## Context

The original single-crate `omiai` grew past what one dependency graph can
express cleanly: the symbolic core, the probabilistic layer, the world
simulator and (planned) an axum server have wildly different dependency
needs, and a single `[dependencies]` section forced every pillar to link
everything. Tests, benches and examples had drifted from APIs
(`persistence` vs `checkpoint`, stale module paths).

## Decision

Root `Cargo.toml` is a **virtual manifest** (`[workspace]`, `members =
["crates/*"]`, resolver 2) with shared `[workspace.dependencies]`. Code
splits into 15 crates layered by dependency:

```
core → knowledge / probabilistic / causal / neuro / memory
     → evolution → io → meta → world
checkpoint ← world (impl lives in checkpoint; orphan-rule friendly)
export / runtime / serve / cli  (thin shells for later slices)
```

No `panic = "abort"` profile override — library crates must not assume
process semantics; panics are catchable at the runtime boundary.

## Consequences

- Each pillar's dependencies are explicit; `cargo tree -p omiai-core`
  is honest.
- Integration tests that span pillars live in the highest crate of the
  chain as dev-dependencies.
- The old binary entry point moved to `omiai-cli`.
- Baseline commit f8920c2 preserves the pre-split state for archaeology.
