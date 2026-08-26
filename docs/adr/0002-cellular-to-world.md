# ADR-0002: Move `cellular` from neuro to a new `omiai-world`

## Context

The original layout put `neuro::cellular` (reversible Margolus block CA)
inside the reservoir-computing crate. But cellular substrates are not
reservoir computing: they are the *environment* other pillars sense and
act on (agents, communication, resource dynamics per the original spec's
world pillar). Keeping them in `omiai-neuro` would force `omiai-world`
to depend on neuro just for one module, or duplicate it.

## Decision

`cellular` moves verbatim to `omiai-world::substrate`. `omiai-neuro`
keeps only reservoir/liquid-state/weights. The bench `cellular.rs`
follows the code into `omiai-world/benches/`.

## Consequences

- Dependency direction is world → causal/probabilistic later, never
  neuro → world.
- One stale import (`neuro::cellular`) had to be fixed in the moved
  bench — caught by clippy during slice 1.
