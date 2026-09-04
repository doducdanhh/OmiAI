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

`cargo test --workspace` currently runs **341 tests across 42 test
targets, all passing**, plus proptests and doc tests. Highlights:

- **`omiai-core`** — full first-order logic stack: Formula/Term AST, CNF
  normalization (NNF → Skolemization → distribution, with constant
  folding), Robinson unification, resolution proofs, DPLL/CDCL, CSP
  solver, LTL. Proptests check CNF preserves ground truth.
- **`omiai-probabilistic`** — Bayesian networks with variable
  elimination; junction-tree inference (Hugin propagation, evidence
  entered before calibration) matching hand-computed exact posteriors
  (P(Rain|Wet)=0.7396 on the textbook network); Gibbs sampling, HMC,
  mean-field VI (with its documented collider limitation), MCTS and
  PUCT-MCTS.
- **`omiai-causal`** — DAGs, back-door criterion, linear SCMs, Pearl
  counterfactuals done properly (abduction recovers noise; intervention
  carries it forward).
- **`omiai-knowledge`** — knowledge graphs, ontology classification with
  disjointness checking, forward/backward chaining, abduction,
  SPARQL-like triple queries, transitive closure.
- **`omiai-neuro`** — seeded sparse reservoirs with RLS readout training
  and spectral-radius normalization (power iteration).
- **`omiai-evolution`** — CGP genetic programming exercised by symbolic
  regression integration tests.
- **`omiai-memory`** — episodic/semantic/working memory stores.
- **`omiai-io`** — rule-based bilingual (EN/VI) NLP front-end turning
  text into logic formulas ("every human is mortal" → ∀x(Human(x)→Mortal(x))).
- **`omiai-world`** — reversible Margolus block cellular automaton with
  HashLife-style caching and rayon sweeps; population conservation is
  proptest-checked. Slice 2 adds the living layer: `FormulaRegistry`
  (generational arena of LTL genomes), atoms with energy metabolism /
  feeding / reproduction, agents that decode a propositional policy from
  their LTL genome over 4-direction observations. Slice 3–4 add emergent
  communication: 6-symbol Lewis signaling game over `airwave`,
  `Vocabulary` accumulating co-occurrence + MI, team reward on MI
  threshold. Slice 5 adds **heritable voice** (`inherit_voice` with
  `VOICE_MUTATION_PROB`), **benefit measurement** (`BenefitCounters`),
  **epoch-based convention tracking** (`ConventionTracker`), and
  **promotion to knowledge graph** (`promote_knowledge` phase) — 3
  consecutive epochs meeting support (16), precision (7/8), and benefit
  (feed-rate cross-multiplied) thresholds promote a convention into a
  named graph node with evidence in its label. World loop is now **8
  phases**: `ca_step → metabolism → speak → agent_act →
  reproduce_and_evolve → team_reward → promote_knowledge → snapshot`.
  Deterministic via ChaCha8Rng (seed/stream/word_pos, ADR-0006).
- **`omiai-checkpoint`** — checkpoint-v1 directory format:
  `Checkpointable` trait, BLAKE3 hashing, atomic writes
  (tmp→fsync→rename→dir-fsync), manifest verification with tamper
  detection, byte-exact CA-grid round-trip + conservation proptests,
  retention window (keep-N recent + milestones), `index.json` with
  rebuild fallback, and the **slice-5 world bundle** — 8 payloads:
  `world/{grid.bin, atoms.cbor, registry.cbor, rng_state.bin, airwave.cbor,
  vocabulary.cbor}` + `communication/conventions.cbor` +
  `knowledge_graph/graph.cbor`. **Backward compatible**: checkpoints from
  slices 2–4 (missing the two new files) load with empty tracker + empty
  graph; `format_version` stays `1`. Bit-exact resume proven by
  round-trip tests.
  Format spec: [`docs/format-spec/checkpoint-v1.md`](docs/format-spec/checkpoint-v1.md).

Integration tests wire pillars together end-to-end (NLP → logic → proof;
causal DAG → SCM → counterfactual; three inference methods agreeing on
the same posterior).

## What's scaffolded (types + doc comments, not yet implemented)

- `omiai-core`: parts of ASP solving and higher-order unification.
- `omiai-knowledge::discocat`, `omiai-probabilistic::{kolmogorov,
  solomonoff}`, `omiai-causal::icp` (narrow), `omiai-memory::procedural`.
- `omiai-export` (model.omiai tar+zstd bundles), `omiai-runtime`
  (`load(bundle)` + step loop), `omiai-serve` (axum `/infer`),
  `omiai-cli` — thin shells awaiting the slices that need them.
- Remaining checkpoint payloads (logic clauses, evolution populations,
  causal DAGs, reservoir weights, active inference beliefs) — world bundle
  with slice-5 extensions is done.

Each scaffold module's doc comment states what it is meant to hold.

## Suggested build order

1. ~~core unification → inference → prover~~ ✅ done and tested
2. ~~knowledge graph + chaining~~ ✅ · probabilistic/causal core ✅ ·
   neuro reservoirs ✅ · evolution CGP ✅ · memory ✅
3. ~~`omiai-world`: agents + world loop over the existing substrate~~ ✅
   done and tested (slice 2)
4. ~~`omiai-world`: emergent communication (speak, airwave, vocabulary,
   team reward)~~ ✅ done and tested (slice 3–4)
5. ~~`omiai-world`: voice inheritance + benefit measurement + convention
   promotion to knowledge graph~~ ✅ done and tested (slice 5)
6. ~~Checkpoint payloads for every pillar + retention policy~~ — world
   bundle (now 8 files incl. conventions + graph) ✅ + retention ✅ ·
   other pillars' payloads remain
7. `omiai-runtime`: deterministic resume from checkpoints
8. `omiai-export` / `omiai-serve` / `omiai-cli`
9. Meta-cognition last, once it has a prover and search worth improving

## Building

```sh
cargo build --workspace
cargo test --workspace          # 276 tests, all passing
cargo clippy --workspace --all-targets
cargo bench -p omiai-world       # or omiai-core / others
```

## Why not just generate all 8 pillars at once?

A few of the spec's own requirements (property-based tests proving
conservation laws, throughput targets, exactness guarantees) are things
that must be *measured*, not asserted. Code that claims to hit them
without benchmarks to back it up would just be guessing. Each slice gets
real tests before the next one leans on it — and this README claims
nothing beyond what the current test suite actually checks.
