# Pillar: world (`omiai-world`)

Status: **partially tested** — `substrate` (reversible Margolus block CA
with HashLife-style block cache, rayon sweeps) has unit tests, an
integration test, a criterion bench, and now a checkpoint round-trip
(see [checkpoint.md](checkpoint.md)). Agents, communication, resources
and the world loop remain **scaffolded** per slice-1 scope.

The Margolus phase alternates per step and conserves population — that
conservation law is proptest-checked across save/load too.
