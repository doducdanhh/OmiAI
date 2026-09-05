# bundle-v1 — single-file export format specification

Status: **implemented** (Slices 11–12). This document matches the code in `crates/omiai-export/src/` and `crates/omiai-runtime/src/` byte for byte; when they disagree, the code is the bug to fix or the spec to bump.

---

## 1. Purpose & distinction from checkpoints

A **bundle** (`model.omiai`) is a **single distributable archive** (tar + zstd) meant for *deployment* and *inference only*.

| Aspect | Checkpoint | Bundle (`model.omiai`) |
|--------|------------|------------------------|
| Form | Directory tree (many files) | One file: `.tar.zst` renamed to `.omiai` |
| Purpose | Resume training / evolution / simulation | Deploy for inference (`load()` + `step()`) |
| Contents | Full state: RNG, airwave, every generation | Curated subset: only what `step()` needs |
| RNG state | Required (bit-exact resume) | Not needed (inference is deterministic given weights) |
| Evolution history | Kept (population, benefit counters) | Dropped (not needed for `step()`) |
| Write frequency | Periodic, automatic, sliding window | Manual, on "release" |
| Versioning | `checkpoint-v1` directory layout | `bundle-v1` archive layout + `manifest.json` |

A bundle is produced by `omiai-export` from a **source checkpoint** (or a live `World` + pillar states). It is consumed by `omiai-runtime` (`load()` → `step()`), `omiai-serve` (`/infer`), and `omiai-cli chat`.

---

## 2. Layout

```
model.omiai    (= model.tar.zst, renamed)
├── manifest.json
├── logic/
│   └── seed_facts.cbor            # minimal ground facts fed to core prover
├── knowledge_graph/
│   └── graph.cbor                 # {concepts: [Concept], relations: [(from,to,kind)]}
├── probabilistic/
│   └── networks.cbor              # [BayesianNetwork] — only if pillar enabled
├── causal/
│   └── dag.cbor                   # CausalDag — only if pillar enabled
├── reservoir/
│   └── weights.safetensors        # {W, W_in, W_out, alpha, spectral_radius} — mmap-friendly
├── world_snapshot/                # OPTIONAL — only if capabilities.world_query = true
│   ├── grid.bin                   # same format as checkpoint-v1 §5
│   ├── atoms.cbor
│   ├── registry.cbor
│   └── vocabulary.cbor            # promoted conventions for fast lookup
├── language_model/                # OPTIONAL — only if capabilities.language_model = true
│   ├── model.gguf                 # quantised weights (not trained inside bundle)
│   ├── tokenizer.json
│   └── model_card.json            # name, version, license, source checksum
└── io/
    └── lexicon.cbor               # extended vocabulary from evolved semantic parser (Slice 8)
```

**File list is authoritative** — every file listed in `manifest.json.files[]` must exist in the archive; extra files not in the manifest are ignored by the loader.

---

## 3. `manifest.json` — mandatory, schema version 1

```json
{
  "format_version": 1,
  "schema": "omiai-bundle",
  "created_utc": "2026-09-05T00:00:00Z",
  "source_checkpoint_step": 1234567,
  "git_commit": "a1b2c3d4",
  "capabilities": {
    "logic": true,
    "knowledge_graph": true,
    "probabilistic": true,
    "causal": true,
    "reservoir": true,
    "world_query": false,
    "language_model": false
  },
  "language_model_info": null,
  "entrypoint": {
    "function": "step",
    "input_schema": "InferInput_v1",
    "output_schema": "InferOutput_v1"
  },
  "files": [
    { "path": "manifest.json", "blake3": "a1b2c3d4e5f6..." },
    { "path": "logic/seed_facts.cbor", "blake3": "f6e5d4c3b2a1..." },
    { "path": "knowledge_graph/graph.cbor", "blake3": "..." },
    { "path": "probabilistic/networks.cbor", "blake3": "..." },
    { "path": "causal/dag.cbor", "blake3": "..." },
    { "path": "reservoir/weights.safetensors", "blake3": "..." },
    { "path": "io/lexicon.cbor", "blake3": "..." }
  ]
}
```

### 3.1 Field table

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `format_version` | `u32` | yes | Schema version. **Must be `1`** for this spec. |
| `schema` | `string` | yes | Fixed string `"omiai-bundle"` — allows tooling to identify the format without extension. |
| `created_utc` | `string` (RFC3339) | yes | Timestamp when bundle was created. |
| `source_checkpoint_step` | `u64` / `null` | yes | Step number of the checkpoint this bundle was exported from (or `null` if built directly). |
| `git_commit` | `string` / `null` | yes | Git SHA of the code that produced this bundle (for reproducibility). |
| `capabilities` | `object` | yes | Boolean flags declaring which pillars are present and usable. **Runtime MUST refuse to call any pillar where the flag is `false`, even if the corresponding directory exists in the archive.** |
| `capabilities.logic` | `bool` | yes | Core prover + seed facts available. |
| `capabilities.knowledge_graph` | `bool` | yes | KnowledgeGraph queryable. |
| `capabilities.probabilistic` | `bool` | yes | BayesianNetwork(s) loadable. |
| `capabilities.causal` | `bool` | yes | CausalDag + do-calculus loadable. |
| `capabilities.reservoir` | `bool` | yes | Reservoir weights loadable for diversity. |
| `capabilities.world_query` | `bool` | yes | `world_snapshot/` present and queryable (read-only). |
| `capabilities.language_model` | `bool` | yes | `language_model/` present. **If `true`, `language_model_info` MUST be non-null.** |
| `language_model_info` | `object` / `null` | conditional | Required iff `capabilities.language_model == true`. See §3.2. |
| `entrypoint` | `object` | yes | Contract for the inference function. |
| `entrypoint.function` | `string` | yes | Fixed to `"step"` — the single exported symbol. |
| `entrypoint.input_schema` | `string` | yes | Name of the input schema version (e.g., `"InferInput_v1"`). |
| `entrypoint.output_schema` | `string` | yes | Name of the output schema version (e.g., `"InferOutput_v1"`). |
| `files[]` | `array` | yes | Array of `{path, blake3}` for **every file in the archive**. Order is not significant but `manifest.json` itself must be first. |

### 3.2 `language_model_info` (required when `capabilities.language_model == true`)

```json
{
  "name": "Qwen2.5-1.5B-Instruct",
  "quantization": "Q4_K_M",
  "license": "Apache-2.0",
  "source_url": "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF",
  "sha256": "d4e5f6a7b8c9...",
  "role": "surface_realization_only",
  "may_assert_unverified_facts": false
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | yes | Human-readable model name (as on Hugging Face). |
| `quantization` | `string` | yes | GGUF quantisation tag (e.g., `Q4_K_M`, `Q5_K_S`, `Q8_0`). |
| `license` | `string` | yes | SPDX licence identifier of the model weights. **Must be OSI-approved** for bundles intended for public distribution. |
| `source_url` | `string` | yes | Canonical download URL for the exact `.gguf` file. |
| `sha256` | `string` (64 hex) | yes | SHA-256 of the `.gguf` file *as stored in the bundle* — allows runtime to verify integrity before loading. |
| `role` | `string` | yes | Fixed to `"surface_realization_only"` — a contract marker. **Runtime MUST enforce** that the model is never asked to *decide* facts, probabilities, or causal claims; it only re-phrases a pre-validated `ReasoningResult`. |
| `may_assert_unverified_facts` | `bool` | yes | Fixed to `false`. If a future slice relaxes this, it must be a major format version bump. |

---

## 4. Per-pillar payload formats

All `.cbor` files use **deterministic CBOR** (canonical encoding: sorted map keys, shortest-form integers, no indefinite-length items) so that BLAKE3 hashes are reproducible across platforms.

### 4.1 `logic/seed_facts.cbor`
```cbor
[  /* array of Formula (same encoding as omiai-core uses in checkpoints) */ ]
```
Minimal ground facts that seed the `TheoremProver` at load time. Typically the "axioms" taught during the session that produced the bundle.

### 4.2 `knowledge_graph/graph.cbor`
```cbor
{
  "concepts": [ {"id": "Human", "label": "Human"}, ... ],
  "relations": [ ["Human", "Mammal", "subclass"], ... ]
}
```
Exact serialisation of `KnowledgeGraph::relations()` and all concepts. **Transitive closure is NOT stored** — runtime recomputes it on load (fast, deterministic).

### 4.3 `probabilistic/networks.cbor`
```cbor
[  /* array of BayesianNetwork */  ]
```
Each `BayesianNetwork` = `{nodes: [Cpt]}` where `Cpt = {variable: str, parents: [str], probs_true: [f64]}`. Matches `omiai_probabilistic::bayesian` serialisation.

### 4.4 `causal/dag.cbor`
```cbor
{
  "children": { "Rain": ["WetGrass"], "Sprinkler": ["WetGrass"] },
  "parents": { "WetGrass": ["Rain", "Sprinkler"] }
}
```
Direct serialisation of `CausalDag` fields.

### 4.5 `reservoir/weights.safetensors`
[SafeTensors](https://github.com/huggingface/safetensors) file — memory-mappable, no pickle risk. Tensors (all `float32`, little-endian):
- `W`            : `[res_size, res_size]` — recurrent matrix (sparse, but stored dense for mmap)
- `W_in`         : `[res_size, in_size]` — input matrix
- `W_out`        : `[out_size, res_size]` — readout weights (trained via RLS/ridge)
- `alpha`        : `[]` scalar — leaking rate
- `spectral_radius`: `[]` scalar — target spectral radius (for verification)

Metadata JSON header stores: `{"res_size": 50, "in_size": 10, "out_size": 5, "density": 0.1}`. Runtime validates shapes on load.

### 4.6 `world_snapshot/` (optional, only if `capabilities.world_query`)

Files use **exact same formats** as `checkpoint-v1` §5 and §5b:
- `grid.bin` — identical to checkpoint (§5)
- `atoms.cbor` — `[Atom]` array
- `registry.cbor` — `FormulaRegistry` serialisation
- `vocabulary.cbor` — `Vocabulary` + `ConventionTracker` (promoted conventions only)

Runtime loads these **read-only** — `step()` is **not** called on the bundled world; it serves only as a queryable knowledge source for `ParseIntent::AskWorld`.

### 4.7 `language_model/` (optional, only if `capabilities.language_model`)

Standard GGUF + tokenizer.json + model_card.json as distributed by Hugging Face. No OmiAI-specific format.

### 4.8 `io/lexicon.cbor` (Slice 8+)
```cbor
[  /* array of [lowercase_label, concept_id] */  ]
```
Extended vocabulary learned by the evolved semantic parser (Slice 8). Allows `NlpParser::load_extended_vocabulary()` to rebuild its `extended_concepts` map exactly.

---

## 5. Load-time contract (mandatory for `omiai-runtime`)

A conforming loader **must** implement the following checks **in order** and fail with a clear error message (not panic) on any violation:

1. **Magic / archive**: File is a valid tar.zst archive. Extract to temporary directory (or stream).
2. **`manifest.json` exists** at archive root.
3. **`format_version == 1`** — if not, error: `"unsupported bundle format version X (this runtime supports 1)"`.
4. **`schema == "omiai-bundle"`** — else error.
5. **All files in `manifest.files[]` exist** in the archive — else error: `"missing file in bundle: <path>"`.
6. **BLAKE3 verification**: For each entry, compute BLAKE3 of the extracted file and compare to `manifest.blake3`. Mismatch → error: `"corrupt file <path>: expected <hash>, got <hash>"`.
7. **Capabilities gate**: For each pillar where `capabilities.X == false`, the loader **must not** attempt to initialise that pillar, even if the corresponding directory exists. (Defence against hand-edited bundles.)
8. **Language model gate**: If `capabilities.language_model == true`:
   - `language_model_info` is non-null and has all required fields.
   - `sha256` of `language_model/model.gguf` matches `language_model_info.sha256`.
   - `license` is in the allow-list configured at runtime build time (default: OSI-approved only).
   - If any check fails → error, do not load model.
9. **Schema validation**: Deserialise each pillar payload. CBOR decode error → error with file + offset.
10. **Entrypoint contract**: Store `entrypoint.input_schema` / `output_schema` for version negotiation with callers.

On success, return a typed handle `Bundle` that implements `step(input: InferInput_v1) -> InferOutput_v1`.

---

## 6. Backward compatibility policy

- `format_version = 1` is the first and only version in this spec.
- When `format_version = 2` is introduced, loaders **must** support reading `v1` bundles:
  - Keep `ManifestV1` struct separate from `ManifestV2`.
  - Provide `upgrade_v1_to_v2(v1: ManifestV1, archive: &mut Archive) -> ManifestV2` that adds defaults for new fields and migrates payloads if needed.
  - Never silently "guess" — the upgrade function is explicit and tested.
- **Breaking changes** (removing a pillar, changing tensor layout, changing CBOR keys) **always** increment `format_version`.
- **Non-breaking additions** (new optional file, new field in `language_model_info` with a default) may stay in the same `format_version` but must be documented in a changelog section of this file.

---

## 7. Differences from `checkpoint-v1` (for reviewers)

| Dimension | `checkpoint-v1` | `bundle-v1` |
|-----------|-----------------|-------------|
| Physical form | Directory | Single `.tar.zst` file (renamed `.omiai`) |
| RNG state | Required (`rng_state.bin`) | **Absent** — inference doesn't resume a stochastic trajectory |
| Evolution history | `evolution/population.cbor`, benefit counters | **Dropped** — not needed for `step()` |
| Airwave / communication buffers | Present (`airwave.cbor`, `conventions.cbor`) | **Dropped** — only *promoted* conventions kept in `knowledge_graph/` + `world_snapshot/vocabulary.cbor` |
| Write path | `Checkpointable::save_step()` (atomic, periodic) | `Export::export_bundle()` (manual, on release) |
| Read path | `load_latest_checkpoint()` → resume `World::step()` | `Bundle::load()` → `Bundle::step()` (no world stepping) |
| Version negotiation | None (single layout) | `entrypoint.input_schema` / `output_schema` for FFI/WASM callers |

---

## 8. Example: creating a bundle programmatically (pseudo-code)

```rust
use omiai_export::Export;
use omiai_checkpoint::load_latest_checkpoint;

let checkpoint = load_latest_checkpoint("checkpoints")?;
let mut export = Export::new()
    .include_logic(true)
    .include_knowledge(true)
    .include_probabilistic(true)
    .include_causal(true)
    .include_reservoir(true)
    .include_world_query(false)      // set true to bundle world_snapshot/
    .include_language_model(false);  // set true + provide model path for Slice 10

export.bundle_from_checkpoint(&checkpoint, "model.omiai")?;
```

The `Export` builder validates that every requested pillar actually has data in the checkpoint before writing; if a pillar is requested but empty, it returns `ExportError::PillarEmpty { pillar }` rather than writing a useless bundle.

---

## 9. Security & supply-chain notes

- **No executable code** in the bundle. SafeTensors + CBOR + GGUF are data-only.
- **BLAKE3 on every file** prevents silent corruption / supply-chain tampering.
- **`language_model_info.license` + `sha256`** allows downstream users to verify they are running exactly the model the bundle author intended, under an acceptable licence.
- **`capabilities` gating** ensures a bundle that claims "no causal reasoning" cannot be tricked into running causal code by a malicious archive edit.

---

## 10. Changelog

| Version | Date | Change |
|---------|------|--------|
| 1 | 2026-09-05 | Initial specification (Slices 11–12). |