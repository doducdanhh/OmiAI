# Pillar: knowledge & ontologies (`omiai-knowledge`)

Status: **tested** — graph/reasoning/ontology/triple/abduction have real
algorithms and tests (forward/backward chaining, classification with
disjointness, SPARQL-like patterns). `discocat` is scaffolded.

## Slice 5: Emergent Convention Promotion → Knowledge Graph

The world loop (`omiai-world::world_loop::promote_knowledge`) promotes
stable, beneficial communication conventions into the graph as **named
concepts with evidence-carrying labels**. This closes the neuro-symbolic
loop: bottom-up emergence → top-down symbolic knowledge.

### What gets promoted
A convention `(symbol s → meaning m*)` is promoted when, in the same
epoch, all hold:
1. **Support**: `s` appears ≥ `MIN_EPOCH_SUPPORT` (16) times.
2. **Precision**: `count(s, m*) / total_s ≥ 7/8` (cross-multiplied in u128).
3. **Benefit**: feed rate when hearing `s` ≥ feed rate when hearing nothing
   (`heard_feeds[s] * quiet_steps ≥ quiet_feeds * heard_steps[s]`,
   cross-multiplied in u128; requires `heard_steps[s] ≥ 8`).
4. **Streak**: same `(s → m*)` has met 1–3 for 3 **consecutive** epochs.

### Graph nodes created
- **Symbol concept**: `symbol_{s}`, label `sym{s}`.
- **State concept**: `state_{meaning_id}` (e.g. `state_res_east`,
  `state_no_resource`, `state_on_resource`), label human-readable.
- **Convention concept**: `convention_sym{s}_{meaning_id}`,
  label **includes the measured evidence**: epoch, precision as fraction
  `hits/total`, feed rates `heard_feeds/heard_steps` vs
  `quiet_feeds/quiet_steps`.

### Relations added
- `convention --signals--> symbol`
- `convention --means--> state`
- `symbol --denotes--> state`

### Checkpoint
The graph is serialized as `knowledge_graph/graph.cbor`:
```cbor
{concepts: [Concept], relations: [(from, to, kind)]}
```
Load is **optional** (backward compatible with slice 2/3/4 checkpoints —
missing file → empty graph). Bit-exact round-trip enforced in
`omiai-checkpoint/tests/world_roundtrip.rs`.

### Causality disclaimer
The benefit criterion measures **conditional correlation**, not causation.
`do_calculus` (later slice) is needed for true causal claims. Node labels
and ADR-0007 explicitly state this.

### RNG note
Promotion consumes **no RNG** (deterministic function of counts), so
resume is bit-exact. However, voice inheritance in `reproduce_and_evolve`
adds RNG draws per arm, changing the trajectory of any pre-slice-5
checkpoint from the first reproduction event onward (ADR-0007).

Notable fixed edge case: transitive closure counts direct self-loop
edges as witnessing (a,a) reachability.
