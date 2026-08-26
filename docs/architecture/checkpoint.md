# Pillar: persistence (`omiai-checkpoint`)

Status: **tested** (slice 1) — see
[format-spec/checkpoint-v1.md](../format-spec/checkpoint-v1.md) for the
byte-exact format. Implemented: `Checkpointable` trait, BLAKE3 hashing,
atomic writes (tmp→fsync→rename→dir-fsync), manifest.json v1,
`verify_dir` tamper detection, `grid.bin` round-trip + conservation
proptests. Sliding retention window and remaining payload types come
with the runtime slice. Legacy JSON persistence kept as `legacy`
(deprecated).

Benchmark note: no throughput numbers are claimed until a criterion
bench measures save/load on realistic grid sizes.
