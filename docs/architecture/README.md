# OmiAI architecture

A zero-training reasoning system in Rust, CPU-only (i7-7700K, 8 GB RAM,
no GPU — hence reservoir computing instead of backpropagation). Eight
research pillars over a symbolic core, split across 15 workspace crates
(see [ADR-0001](../adr/0001-workspace-layout.md)):

```
                 ┌─────────────┐
   text ──► io   │  omiai-io   │  NLP → logic formulas
                 └──────┬──────┘
                        ▼
   ┌────────── core ──────────┐   knowledge / probabilistic /
   │ logic · CNF · unification│   causal / neuro / memory
   │ resolution · DPLL/CDCL   │   (pillars, siblings over core)
   │ CSP · prover · LTL       │
   └──────────────────────────┘
                        ▼
              evolution → meta          world (substrate)
                        ▼                   ▼
                   checkpoint ◄────────────┘
                        ▼
           export · runtime · serve · cli
```

Status vocabulary is deliberate: **tested** means real assertions on
real algorithms; **scaffolded** means types/signatures exist with
documented intent but `todo!()` bodies. Nothing here claims a
performance number without a criterion benchmark.

| doc | pillar | status |
|---|---|---|
| [core.md](core.md) | symbolic reasoning | tested |
| [knowledge.md](knowledge.md) | graphs & ontologies | tested |
| [probabilistic.md](probabilistic.md) | Bayesian / sampling | tested |
| [causal.md](causal.md) | causality | tested |
| [neuro.md](neuro.md) | reservoir computing | tested |
| [memory.md](memory.md) | episodic/semantic memory | tested |
| [world.md](world.md) | cellular substrate & agents | partially tested; agents scaffolded |
| [evolution.md](evolution.md) | genetic programming | tested |
| [checkpoint.md](checkpoint.md) | persistence | tested (v1 grid payload) |
