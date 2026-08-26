# checkpoint-v1 — directory format specification

Status: **implemented and tested** (`omiai-checkpoint` crate, slices 1–2:
grid payload, world bundle, index, retention; logic/knowledge/reservoir
payloads remain later slices). This document matches the code in
`crates/omiai-checkpoint/src/` byte for byte; when they disagree, the
code is the bug to fix or the spec to bump.

## 1. Layout

A checkpoint is a **directory**, never a single file:

```
checkpoints/
├── index.json                    # implemented: valid checkpoints, ascending step
└── step_00001234/
    ├── manifest.json
    ├── world/                    # implemented (slice 2) — see §5b
    │   ├── grid.bin              # world CA grid
    │   ├── atoms.cbor
    │   ├── registry.cbor
    │   └── rng_state.bin
    ├── logic/clauses.cbor        # (later slices)
    ├── knowledge_graph/graph.cbor
    ├── evolution/population.cbor
    ├── causal/dag.cbor
    └── ...
```

`step_XXXXXXXX` uses zero-padded 8-digit step numbers
(`omiai_checkpoint::index::list_steps` discovers them by prefix scan).

### 1b. `index.json` — implemented

- Written atomically via `write_atomic`; content: `{entries: [{step,
  dir}]}` ascending by step.
- Load: missing or corrupt → rebuild from a `list_steps` directory scan;
  `read_or_rebuild_index(root)` returns `(index, rebuilt)` so callers
  know it was reconstructed (no panic, no total silence).

### 1c. Retention window — implemented

- `RetentionPolicy { keep_recent: 10, milestone_every: 100 }` (defaults).
- `apply_retention(root, policy)` deletes non-recent, non-milestone
  `step_*` directories and returns the removed `(step, path)` pairs
  sorted ascending. Milestones (steps divisible by `milestone_every`)
  are never deleted.

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
renamed, never overwriting old checkpoints) lands with the runtime
crate; the retention window itself is implemented (§1c).

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
20..          body: row-major cells, ONE BYTE per cell
```

- Header is 20 bytes total. (Slice-1's plan said "16-byte header", but
  the listed fields sum to 20; the field list wins.)
- Body length is exactly `width*height` bytes; each byte holds the raw
  cell state (`0..num_states`). Slice-1 shipped a bit-packed body
  ("bit set = live"), which cannot represent the resource states 2/3 of
  the slice-2 world — bit-exact resume requires the full value, so the
  body was upgraded to one byte per cell while the magic stayed
  unchanged. Readers of the old format fail on the body-length check.
- Dimensions above `u16::MAX` → `CheckpointError::GridTooLarge`.
- Load checks magic (`BadMagic`), then body length and per-cell range,
  after verifying the BLAKE3 hash from the manifest.
- **Phase is not persistent state**: the Margolus partition phase and
  the HashLife-style block cache are private bookkeeping in
  `omiai-world` and reset on load. A resumed run replays determinism
  through `rng_seed`/`rng_state_hex` plus the grid, not the phase.

## 5b. `world/` — full world bundle (slice 2, implemented)

`impl Checkpointable for World` (in `world_bundle.rs`) writes four files
under `world/`, each hashed into `manifest.json` as `world/<name>`:

| file | content |
|---|---|
| `grid.bin` | §5 format (1 byte/cell body) |
| `atoms.cbor` | CBOR `{step_count: u64, atoms: [{pos, energy, gene, age}]}` |
| `registry.cbor` | CBOR `{genomes: [Genome]}` theo thứ tự slot |
| `rng_state.bin` | 32 bytes: u64 LE seed + u64 LE stream + u128 LE word_pos |

- RNG resume (ADR-0006): `ChaCha8Rng::seed_from_u64(seed)` →
  `set_stream(stream)` → `set_word_pos(word_pos)`.
- Load verifies manifest version + every BLAKE3 hash before trusting any
  payload; a wrong rng_state length is `Corrupt`.
- Load also checks **cross-payload referential integrity**: every atom
  position must be inside the loaded grid and every `gene` slot must
  exist in `registry.cbor`. A dangling reference is `Corrupt`, not a
  silently inert atom (§4: stop loudly, never skip).
- Bit-exact resume is test-enforced (`tests/world_roundtrip.rs`).

## 6. Compatibility policy

- A **v2 reader MUST read v1** directories.
- A v1 writer never emits fields outside this schema.
- Adding a new optional field = minor bump inside `format_version`
  encoding (e.g. `1_001`); changing/removing a field or altering any
  byte layout here = major bump to `2`, with a migration note.
