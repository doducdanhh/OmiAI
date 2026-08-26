# Pillar: probabilistic reasoning (`omiai-probabilistic`)

Status: **tested** — Bayesian networks (variable elimination), junction
tree (Hugin propagation with evidence entry before calibration), Gibbs
sampling, HMC, mean-field VI, MCTS + PUCT-MCTS, Markov/POMDP helpers;
`kolmogorov`/`solomonoff` scaffolded.

Known honest limitation: fully factorized mean-field VI cannot represent
explaining-away across colliders — on the rain/sprinkler network it
undershoots the exact posterior (0.617 vs 0.7396); the integration test
asserts a band, not equality.
