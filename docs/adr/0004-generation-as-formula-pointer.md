# ADR-0004: World generation as `Formula` pointers (orientation for omiai-world)

## Context

The original spec's world pillar describes generated environments
(terrain, resources, hazards) that agents must *reason about*. Two
options: generate concrete grids and let agents perceive raw cells, or
generate symbolic descriptions the logic engine can consume directly.

## Decision

World generation is oriented toward producing **`omiai_core::Formula`
descriptions alongside the concrete substrate**: a cell enum (empty /
resource / hazard / …) is small enough to encode both ways, and keeping
the symbolic view first-class means perception → logic needs no
translation layer. The CA grid checkpoint (`grid.bin`) persists only
the concrete substrate; the symbolic description is regenerated
deterministically from `rng_seed`.

## Consequences

- `omiai-world` will depend on `omiai-core` (for `Formula`) — accepted,
  it sits above core in the layering anyway.
- Nothing in slice 1 implements this yet; it constrains the world API
  design in the next slice. Recorded now so the checkpoint format
  (concrete-only payloads) doesn't get "fixed" later into something
  that fights it.
