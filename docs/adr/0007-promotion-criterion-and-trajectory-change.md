# ADR-0007: Convention Promotion Criterion, Integer Thresholds, and Accepted Trajectory Change

## Status
Accepted (implemented in slice 5)

## Context
Slice 5 introduces the mechanism that promotes emergent communication
conventions into the symbolic knowledge graph (`knowledge::graph`). A
convention is a stable pairing `(symbol → meaning)` that has **demonstrably
shown stable benefit over enough generations according to a clear
statistical threshold** (per the project mandate). This ADR records the
chosen criterion, its implementation with exact integer arithmetic, and the
one unavoidable behavioral consequence: pre-slice-5 checkpoint trajectories
diverge at resume.

## Decision

### 1. Promotion criterion = conditional benefit + precision + support + streak
A convention `(s → m*)` is promoted iff **all** hold in the same epoch:
- **Support**: symbol `s` appears at least `MIN_EPOCH_SUPPORT = 16` times
  in the epoch's `Vocabulary` (otherwise "100% precision" is just 1/1).
- **Precision**: `count(s, m*) * PRECISION_DEN ≥ total_s * PRECISION_NUM`
  with `PRECISION_NUM/PRECISION_DEN = 7/8`. Cross-multiplied in `u128`
  — no floating point, no rounding differences across machines.
- **Benefit**: the feed rate when hearing `s` is **not worse** than the feed
  rate when hearing nothing:
  `heard_feeds[s] * quiet_steps ≥ quiet_feeds * heard_steps[s]`
  (again `u128` cross-multiplication). Requires `heard_steps[s] ≥
  MIN_BENEFIT_SUPPORT = 8`. If `quiet_steps == 0`, benefit holds iff
  `heard_feeds[s] > 0`.
- **Streak**: the same `(s → m*)` has met the above for
  `PROMOTION_EPOCHS = 3` **consecutive** epochs. A change of meaning
  resets the streak to 1; a silent/failing epoch resets it to 0.

All thresholds live in `ecology.rs` so tests can override them with small
values.

### 2. Benefit is *correlation*, not causation — `do_calculus` is a later slice
The criterion measures `P(feed | hear s) ≥ P(feed | hear nothing)`. This
is a conditional correlation. It does **not** claim that emitting `s`
*causes* feeding. Causal identification (`do_calculus`) is deferred to a
future slice. The graph node label and this ADR explicitly say so.

### 3. Integer arithmetic = deterministic decisions
All divisions are cross-multiplied in `u128`. The same count table always
yields the same promote/don't-promote decision, independent of CPU, Rust
version, or floating-point rounding mode. This is required for bit-exact
resume from checkpoints.

### 4. Voice inheritance changes the RNG draw order
Before slice 5, `reproduce_and_evolve` gave every child `voice:
Vec::new()`. Slice 5 wires `inherit_voice(parent_voice, registry, rng)`,
which draws `f64` per arm (`N_SYMBOLS` arms per child) to decide mutation.
Therefore the RNG stream position after one generation differs from the
pre-slice-5 run. **Consequence**: a checkpoint saved by slice 4, when
loaded and resumed by slice 5, will **diverge from the original
pre-slice-5 trajectory** from the first reproduction event onward. This is
documented as a contract in slice-5 spec §2 and is **not a bug** — it is
the necessary cost of making conventions heritable. The checkpoint format
version is not bumped because old checkpoints still load correctly (the
new payloads are optional).

### 5. Knowledge graph nodes carry their evidence
The promoted convention becomes a concept node with id
`convention_sym{s}_{meaning_id}` and a label that **includes the measured
numbers**: epoch, precision as a fraction `hits/total`, feed rates
`heard_feeds/heard_steps` vs `quiet_feeds/quiet_steps`. This makes the
graph auditable — reading the node tells you *why* it was promoted.
Relations added: `convention --signals--> symbol`, `convention
--means--> state`, `symbol --denotes--> state`.

## Alternatives Considered
- **Float thresholds**: rejected because float equality is not portable
  and would break bit-exact resume.
- **Full causal criterion (do-calculus)**: rejected as out of scope for
  this slice; would require `causal::do_calculus` which is not built yet.
- **Bumping `format_version` for the optional payloads**: rejected
  because the old schema is a valid subset; loaders that don't know the
  new files simply hash what's listed in the manifest and succeed. The
  new loader defaults the missing pieces. This is logically a minor
  extension, same as "new optional field in existing payload".

## Consequences
- **Positive**: conventions that survive the filter are genuinely useful
  (better feed rate) and stable (3 epochs), and their evidence is
  permanently recorded in the graph.
- **Positive**: deterministic across machines and checkpoint resumptions.
- **Negative**: existing slice-4 checkpoints resume on a different
  trajectory (documented, accepted, test-enforced bit-exact within the
  same version).
- **Negative**: the benefit criterion can promote spurious correlations
  (a symbol that happens to be heard near food but doesn't help find it).
  The streak and precision filters mitigate this; `do_calculus` will
  address it properly later.

## References
- Slice-5 design spec: `docs/superpowers/specs/2026-08-30-world-knowledge-promotion-slice5-design.md`
- Implementation: `crates/omiai-world/src/communication.rs` (`BenefitCounters`, `ConventionTracker`), `crates/omiai-world/src/world_loop.rs` (`promote_knowledge`, `inherit_voice`)
- Checkpoint: `crates/omiai-checkpoint/src/world_bundle.rs`
- Tests: `crates/omiai-checkpoint/tests/world_roundtrip.rs` (bit-exact + backward compat)