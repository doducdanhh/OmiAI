# OmiAI

A zero-training, self-bootstrapping reasoning system in Rust. No deep
learning, no GPU, no PyTorch/TensorFlow/JAX, no training datasets —
instead, eight pillars of classical/theoretical CS and cognitive science:
symbolic logic & formal reasoning, knowledge graphs & ontologies,
evolutionary computation, reservoir computing, cellular automata,
Bayesian/causal inference, active inference (free energy principle), and
neuro-symbolic search.

Built incrementally in reviewable slices: real working code and real
tests for what's done, honest scaffolding (exact types/signatures,
documented intent) for the rest. This repo is a **Cargo workspace of 15
crates** (see `docs/adr/0001-workspace-layout.md`).

Target hardware: CPU-only, 8 GB RAM (i7-7700K class). That constraint is
a design input, not an apology: reservoir computing instead of
backpropagation, criterion benchmarks before any performance claim.

## Status: what's actually implemented (and tested)

`cargo test --workspace` currently runs **358 tests across 52 test
targets, all passing**, plus proptests and doc tests. Highlights:

- **`omiai-core`** — full first-order logic stack: Formula/Term AST, CNF
  normalization (NNF → Skolemization → distribution, with constant
  folding), Robinson unification, resolution proofs, DPLL/CDCL, CSP
  solver, LTL. Proptests check CNF preserves ground truth. **57 tests**.
- **`omiai-probabilistic`** — Bayesian networks with variable
  elimination; junction-tree inference (Hugin propagation, evidence
  entered before calibration) matching hand-computed exact posteriors
  (P(Rain|Wet)=0.7396 on the textbook network); Gibbs sampling, HMC,
  mean-field VI (with its documented collider limitation), MCTS and
  PUCT-MCTS. **30 tests**.
- **`omiai-causal`** — DAGs, back-door criterion, linear SCMs, Pearl
  counterfactuals done properly (abduction recovers noise; intervention
  carries it forward), ICP. **17 tests**.
- **`omiai-knowledge`** — knowledge graphs over petgraph: concepts,
  relations, path queries, transitive closure, forward/backward chaining,
  abductive reasoning, ontology (subclass/disjoint), DisCoCat, triple
  store, SPARQL-like queries. **20 tests**.
- **`omiai-neuro`** — echo-state network with RLS readout (no backprop,
  CPU-friendly), liquid state machines, spectral normalization helpers.
  **4 tests**.
- **`omiai-evolution`** — Cartesian Genetic Programming (CGP) with async
  island model; genetic programming directly on logic ASTs (Formula GP
  on `omiai_core::logic_engine::Formula`, LTL Formula GP on
  `omiai_core::ltl::LtlFormula`). **17 tests**.
- **`omiai-world`** — artificial life / open-ended evolution substrate:
  reversible block cellular automaton (Margolus neighbourhood, rayon
  parallel sweep, HashLife-style block cache); atoms = energy +
  coordinate + gene (FormulaId pointer into registry); agents decode LTL
  formulae into move/speak policies; Lewis signaling-game communication
  with mutual-information measurement; convention promotion to
  `knowledge::graph` via exact integer thresholds (7/8 precision, 3-epoch
  streak); central 5-phase world loop (CA step, metabolism, agent act,
  reproduce+evolve, snapshot). **99 unit tests + 1 property test**.
- **`omiai-checkpoint`** — versioned directory-format checkpoints for
  long-running sessions: `Checkpointable` trait, atomic write (tmp→fsync→rename), manifest with
  git commit + full RNG state + BLAKE3 hashes, sliding retention window
  + permanent milestones, `index.json` for resume. Round-trip tests for
  CA grid, world bundle, communication types. **13 tests**.
- **`omiai-memory`** — working memory, episodic buffer, consolidation.
  **7 tests**.
- **`omiai-meta`** — introspection (proof explanations), Active Inference
  self-improvement loop (variational free energy), autopoiesis
  (self-production coupling GP + knowledge graph), hierarchical goal
  system, continual learning. **5 tests**.
- **`omiai-io`** — conversation, action, chat, session, policy, tools.
  **8 tests**.
- **`omiai-export`** — bundle export format scaffold (2 tests).
- **`omiai-runtime`** — **SCAFFOLD ONLY** (minimal load-and-infer crate
  for `model.omiai` bundles, targeting native/cdylib/WASM; no
  implementation yet).
- **`omiai-serve`** — **SCAFFOLD ONLY** (axum HTTP server for
  `POST /infer`; no implementation yet).
- **`omiai-cli`** — CLI entry points (train/resume/export/bench). **2 tests**.

**Not yet implemented (scaffolds only):**
- `omiai-runtime` — load/step runtime for exported bundles (native,
  cdylib, wasm32-wasi, wasm32-unknown-unknown)
- `omiai-serve` — HTTP inference server
- `omiai-export` — full bundle export (`model.omiai` = zstd tar with
  manifest + pruned payloads)
- Bundle format specification (`docs/format-spec/bundle-v1.md`)
- Root-level `benches/`, `examples/`, `scripts/`, `.github/workflows/`
- Integration tests in root `tests/`

## Suggested build order (respects dependencies)

1. `omiai-core` (logic, unification, inference, CSP, prover, LTL) ✓
2. `omiai-probabilistic` / `omiai-causal` (independent, parallel) ✓
3. `omiai-knowledge` (graph, reasoning, ontology) ✓
4. `omiai-neuro` (reservoir) ✓
5. `omiai-evolution` (CGP + Formula GP) ✓
6. `omiai-world` (substrate, atoms, agents, communication, world loop) ✓
7. `omiai-checkpoint` (Checkpointable + round-trip for all above) ✓
8. `omiai-memory` / `omiai-meta` / `omiai-io` (independent) ✓
9. `omiai-export` (bundle format + pruning logic) — **NEXT**
10. `omiai-runtime` (load/step, WASM/FFI targets) — **AFTER export**
11. `omiai-serve` (axum `/infer`) — **AFTER runtime**
12. `omiai-cli` (tying it all together) — **LAST**

Each slice is "done" when it has unit tests, proptests where applicable,
and a criterion benchmark for performance-critical paths.

## Quick start

```bash
# Run all tests
cargo test --workspace

# Run the logic demo
cargo run --example logic_demo

# Benchmarks (run from crate root)
cargo bench --bench cnf      # omiai-core CNF normalization
cargo bench --bench sat      # omiai-core DPLL/CDCL
cargo bench --bench cgp      # omiai-evolution CGP
cargo bench --bench knowledge # omiai-knowledge graph queries
cargo bench --bench reservoir # omiai-neuro ESN+RLS
cargo bench --bench bayesian  # omiai-probabilistic junction tree
cargo bench --bench cellular  # omiai-world CA step throughput
```

## Architecture Decision Records

- `docs/adr/0001-workspace-layout.md` — 15-crate virtual workspace
- `docs/adr/0002-cellular-to-world.md` — CA moved from neuro to world
- `docs/adr/0003-checkpoint-directory-blake3.md` — directory checkpoints + BLAKE3
- `docs/adr/0004-generation-as-formula-pointer.md` — gene = FormulaId, not raw Formula
- `docs/adr/0005-io-meta-cycle.md` — IO/Meta coupling
- `docs/adr/0006-world-rng-chacha8.md` — ChaCha8RNG with stream/word_pos for bit-exact resume
- `docs/adr/0007-promotion-criterion-and-trajectory-change.md` — convention promotion criterion (7/8 precision, 3-epoch streak, integer arithmetic)

## Checkpoint format specification

`docs/format-spec/checkpoint-v1.md` — directory layout, manifest, atomic write, retention, index, per-pillar payloads (world, communication, knowledge_graph, logic, evolution, reservoir, causal).

Bundle format specification (`docs/format-spec/bundle-v1.md`) — **TODO** (after export crate is complete).

## Hardware assumptions

- CPU: Intel Core i7-7700K (4 cores / 8 threads, AVX2)
- RAM: 8 GB total (Rust + OS + data must fit)
- No discrete GPU
- Disk: SSD preferred for checkpoint I/O

All numerical code uses `f64`; no `f16`/`bf16` SIMD paths. Reservoir
readout is trained by RLS (O(n²) memory in reservoir size) — sizes are
kept ≤ 2000 units to stay within RAM. CA grid uses 1 byte/cell; a
1024×1024 grid ≈ 1 MB. Checkpoint payloads use CBOR (ciborium) for
structured data, raw binary for grids, safetensors for tensors (mmap
friendly).

## License

MIT OR Apache-2.0