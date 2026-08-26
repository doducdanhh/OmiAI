# Pillar: knowledge & ontologies (`omiai-knowledge`)

Status: **tested** — graph/reasoning/ontology/triple/abduction have real
algorithms and tests (forward/backward chaining, classification with
disjointness, SPARQL-like patterns). `discocat` is scaffolded.

Notable fixed edge case: transitive closure counts direct self-loop
edges as witnessing (a,a) reachability.
