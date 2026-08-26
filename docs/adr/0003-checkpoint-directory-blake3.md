# ADR-0003: Checkpoints are directories with BLAKE3 manifests, not single files

## Context

Long-running evolution/training sessions need crash-safe resume. The
legacy `persistence.rs` serialized one JSON/CBOR envelope per save —
fine for small state, but it cannot verify partial corruption, cannot
add a payload type without rewriting the whole file format, and a
truncated write loses everything.

## Decision

checkpoint-v1 is a **directory** (`step_XXXXXXXX/`) with a
`manifest.json` pinning `format_version`, provenance (git commit,
timestamp), full RNG state (`rng_seed`, `rng_state_hex` — deterministic
resume) and per-file BLAKE3 hashes. Every file is written via tmp →
fsync → rename → dir-fsync. Verification re-hashes each payload and
fails loudly on mismatch (`CheckpointError::Corrupt`). See
[format spec](../format-spec/checkpoint-v1.md).

Rejected alternatives: single bundle file (no partial verification, no
per-type evolution), checksums weaker than BLAKE3 (speed on multi-MB
grids matters on CPU-only hardware).

## Consequences

- Adding a new payload (e.g. `causal/dag.cbor`) is additive: new file +
  manifest row, no format break.
- The legacy JSON path survives as `omiai_checkpoint::legacy`,
  deprecated, until all callers migrate.
