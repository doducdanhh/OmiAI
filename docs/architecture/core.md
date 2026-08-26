# Pillar: symbolic core (`omiai-core`)

Status: **tested** — unit tests, integration tests (`integration_v2/v3`),
proptests, and criterion benches (`cnf`, `sat`).

- `logic_engine` — Formula/Term AST, NNF/Skolemization/CNF pipeline,
  constant folding; ground evaluation.
- `substitution`, `unification` — Robinson unification with occurs check.
- `inference` — resolution proof search, DPLL/CDCL SAT.
- `csp_solver`, `prover`, `asp_solver`, `ltl`,
  `higher_order_unification`.
- `utils/{arena,stats,serialization}`.

Verified semantics include: CNF preserves ground truth on constants
(True → empty clause set, False → one empty clause); P(R|W) via
junction tree matches hand-computed 0.7396 (see probabilistic.md).
