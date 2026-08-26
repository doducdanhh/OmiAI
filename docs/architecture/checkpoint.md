# Pillar: persistence (`omiai-checkpoint`)

Status: **implemented and tested** (slices 1–2) — see
[format-spec/checkpoint-v1.md](../format-spec/checkpoint-v1.md) for the
byte-exact format. Implemented: `Checkpointable` trait, BLAKE3 hashing,
atomic writes (tmp→fsync→rename→dir-fsync), manifest.json v1,
`verify_dir` tamper detection, `grid.bin` round-trip + conservation
proptests, sliding retention window (`RetentionPolicy`, `apply_retention`
— keep-N recent + permanent milestones), `index.json` atomic write with
fallback rebuild (`read_or_rebuild_index`), and the slice-2 world bundle:
4-file `world/` payload (grid, atoms.cbor, registry.cbor,
rng_state.bin) with **bit-exact resume** — save at step N, load, run M
steps, and the state matches a world that ran N+M continuously (proven
by test). RNG resume via (seed, stream, word_pos), ADR-0006.
Legacy JSON persistence kept as `legacy` (deprecated).

Benchmark note: no throughput numbers are claimed until a criterion
bench measures save/load on realistic grid sizes.
