# checkpoint-v1 — directory format specification

Status: **implemented and tested** (`omiai-checkpoint` crate, slice 1).
This document matches the code in `crates/omiai-checkpoint/src/` byte for
byte; when they disagree, the code is the bug to fix or the spec to bump.

## 1. Layout

A checkpoint is a **directory**, never a single file:

```
checkpoints/
├── index.json                    # (later slice) valid checkpoints, ascending step
└── step_00001234/
    ├── manifest.json
    ├── grid.bin                  # world CA grid (the only payload in v1)
    ├── logic/clauses.cbor        # (later slices)
    ├── knowledge_graph/graph.cbor
    ├── evolution/population.cbor
    ├── causal/dag.cbor
    └── ...
```

`step_XXXXXXXX` uses zero-padded 8-digit step numbers
(`omiai_checkpoint::index::list_steps` discovers them by prefix scan).

## 2. manifest.json schema

```json
{
  "format_version": 1,
  "git_commit": null,
  "step": 0,
  "timestamp_utc": "",
  "rng_seed": 0,
  "rng_state_hex": "",
  "files": [
    { "path": "grid.bin", "blake3": "<64 lowercase hex chars>" }
  ]
}
```

| field | type | meaning |
|---|---|---|
| `format_version` | u32 | `1`; a loader must reject unknown versions with an error, never silently adapt |
| `git_commit` | string? | build provenance; read from `OMIAI_GIT_COMMIT` env at compile time when set |
| `step` | u64 | logical simulation step at save time (v1 writers emit `0`; the runtime crate fills it) |
| `timestamp_utc` | string | RFC 3339 UTC creation time |
| `rng_seed` | u64 | seed of the RNG that produced the persisted state |
| `rng_state_hex` | string | opaque serialized RNG stream state, so resume reproduces the exact trajectory |
| `files` | array | one `{path, blake3}` per payload file; `path` is relative to the checkpoint directory |

Missing required fields are an error (`CheckpointError::MissingField`) —
never defaulted silently.

## 3. Atomic write protocol

Implemented in `omiai_checkpoint::write_atomic`, used for every file:

1. Write bytes to a hidden temp sibling `<dir>/.<name>.tmp`.
2. `sync_all()` the temp file (contents + metadata durable).
3. `rename()` tmp → target (atomic on POSIX).
4. Open the parent directory and `sync_all()` it, so the rename entry
   itself survives power loss (unix).

On success no `.tmp` residue remains (tested). The directory-level
protocol (whole `step_XXXXXXXX/` written as `.tmp_step_XXXXXXXX/` then
renamed, sliding retention window of N recent + milestones every K
steps, never overwriting old checkpoints) lands with the runtime crate.

## 4. Verification

`verify_dir(dir)` reads `manifest.json` and re-hashes every recorded
file with BLAKE3. Any mismatch → `CheckpointError::Corrupt { path,
expected, actual }`. Resume must stop loudly on corruption — never skip.

## 5. `grid.bin` — world CA grid format

Little-endian throughout.

```
offset  size  field
0       10    magic "OMICAGRID\0"   (ASCII, NUL-terminated)
10      2     width      u16 LE
12      2     height     u16 LE
14      1     num_states u8
15      1     flags      u8   (= 0)
16      4     reserved   u32 LE (= 0)
20..          body: bit-packed cells, row-major, LSB-first
```

- Header is 20 bytes total. (Slice-1's plan said "16-byte header", but
  the listed fields sum to 20; the field list wins.)
- Body length is `ceil(width*height / 8)` bytes. Cell value 0 = empty;
  bit set = live (state 1). Multi-state grids are not yet encoded — the
  bit encodes "nonzero".
- Dimensions above `u16::MAX` → `CheckpointError::GridTooLarge`.
- Load checks magic (`BadMagic`), then body length, after verifying the
  BLAKE3 hash from the manifest.
- **Phase is not persistent state**: the Margolus partition phase and
  the HashLife-style block cache are private bookkeeping in
  `omiai-world` and reset on load. A resumed run replays determinism
  through `rng_seed`/`rng_state_hex` plus the grid, not the phase.

## 6. Compatibility policy

- A **v2 reader MUST read v1** directories.
- A v1 writer never emits fields outside this schema.
- Adding a new optional field = minor bump inside `format_version`
  encoding (e.g. `1_001`); changing/removing a field or altering any
  byte layout here = major bump to `2`, with a migration note.
