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

`cargo test --workspace` currently runs **272 tests across 38 test
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
  their LTL genome over 4-direction observations, the deterministic
  5-phase world loop (ChaCha8-seeded — same seed ⇒ bit-exact trajectory),
  arity-preserving mutation, and world-invariant proptests.
- **`omiai-checkpoint`** — checkpoint-v1 directory format:
  `Checkpointable` trait, BLAKE3 hashing, atomic writes
  (tmp→fsync→rename→dir-fsync), manifest verification with tamper
  detection, byte-exact CA-grid round-trip + conservation proptests,
  retention window (keep-N recent + milestones), `index.json` with
  rebuild fallback, and the slice-2 world bundle — grid + atoms +
  registry + RNG state with **bit-exact resume** proven by round-trip
  test.
  Format spec: [`docs/format-spec/checkpoint-v1.md`](docs/format-spec/checkpoint-v1.md).

Integration tests wire pillars together end-to-end (NLP → logic → proof;
causal DAG → SCM → counterfactual; three inference methods agreeing on
the same posterior).

## What's scaffolded (types + doc comments, not yet implemented)

- `omiai-world`: communication and multi-species ecology — the substrate,
  agents, world loop and checkpoint resume are real; inter-agent
  signaling is not.
- `omiai-core`: parts of ASP solving and higher-order unification.
- `omiai-knowledge::discocat`, `omiai-probabilistic::{kolmogorov,
  solomonoff}`, `omiai-causal::icp` (narrow), `omiai-memory::procedural`.
- `omiai-export` (model.omiai tar+zstd bundles), `omiai-runtime`
  (`load(bundle)` + step loop), `omiai-serve` (axum `/infer`),
  `omiai-cli` — thin shells awaiting the slices that need them.
- Remaining checkpoint payloads (logic clauses, graphs, populations,
  DAGs) — grid + world bundle are done.

Each scaffold module's doc comment states what it is meant to hold.

## Suggested build order

1. ~~core unification → inference → prover~~ ✅ done and tested
2. ~~knowledge graph + chaining~~ ✅ · probabilistic/causal core ✅ ·
   neuro reservoirs ✅ · evolution CGP ✅ · memory ✅
3. ~~`omiai-world`: agents + world loop over the existing substrate~~ ✅
   done and tested
4. ~~Checkpoint payloads for every pillar + retention policy~~ — world
   bundle ✅ + retention ✅ · other pillars' payloads remain
5. `omiai-runtime`: deterministic resume from checkpoints
6. `omiai-export` / `omiai-serve` / `omiai-cli`
7. Meta-cognition last, once it has a prover and search worth improving

## Building

```sh
cargo build --workspace
cargo test --workspace          # 272+ tests
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
