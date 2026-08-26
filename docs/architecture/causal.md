# Pillar: causality (`omiai-causal`)

Status: **tested** — DAGs, back-door criterion, linear SCMs, Pearl
counterfactuals (abduction → action → prediction), ICP scaffolded to
real but narrow scope.

Counterfactual semantics are tested with noise *and* without:
observation Y=10 with model value 2.85 abduces u_Y=7.15, so do(X:=0)
yields Y'=8.0 — not the noise-free 0.85.
