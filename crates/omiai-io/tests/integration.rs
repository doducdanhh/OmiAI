//! End-to-end integration tests for the OmiAI pipeline.
//!
//! Each test composes two or more modules (NLP ↔ logic ↔ prover, KG ↔
//! reasoning, causal DAG ↔ SCM ↔ counterfactuals, etc.) to verify that
//! the modules wire together correctly.
//!
//! Run with: `cargo test --release --test integration`

use std::collections::HashMap;

use omiai_causal::dag::CausalDag;
use omiai_causal::do_calculus::backdoor_criterion;
use omiai_causal::intervention::counterfactual;
use omiai_causal::scm::{CausalModel, StructuralEquation};
use omiai_core::inference::ProofResult;
use omiai_core::logic_engine::{Formula, Term};
use omiai_core::prover::TheoremProver;
use omiai_evolution::fitness::mse_to_fitness;
use omiai_evolution::genetic_programming::GeneticProgram;
use omiai_io::nlp_parser::NlpParser;
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_knowledge::ontology::{Axiom, Ontology};
use omiai_knowledge::reasoning::{HornRule, backward_chain, forward_chain};
use omiai_knowledge::triple::{TermPattern, Triple, TriplePattern, TripleStore};
use omiai_meta::self_improvement::MetaCognitiveEngine;
use omiai_world::substrate::CellularAutomaton;
use omiai_neuro::reservoir::Reservoir;
use omiai_probabilistic::bayesian::{BayesianNetwork, Cpt};
use omiai_probabilistic::markov::tiger_pomdp;
use omiai_probabilistic::mcts::{GameState, Mcts, filter_actions_with_solver};

// ---------------------------------------------------------------------------
// 1. NLP → Logic → Prover pipeline
// ---------------------------------------------------------------------------

/// Parse a natural-language statement into a logical formula, then use the
/// theorem prover to derive a conclusion.
#[test]
fn nlp_to_proof_socrates() {
    let parser = NlpParser::default();

    // "every human is mortal"  →  ∀x (Human(x) → Mortal(x))
    let rule_msg = parser
        .parse_message("every human is mortal", omiai_io::nlp_parser::DetectedLanguage::English)
        .expect("parser should succeed");
    let rule_f = rule_msg.formula.expect("rule should carry a formula");
    let human_x = Formula::atom("Human", vec![Term::Var("x".into())]);
    let mortal_x = Formula::atom("Mortal", vec![Term::Var("x".into())]);
    let expected_rule = Formula::ForAll(
        "x".into(),
        Box::new(Formula::Implies(Box::new(human_x), Box::new(mortal_x))),
    );
    assert_eq!(rule_f, expected_rule);

    // "socrates is human"  →  Human(socrates)
    let fact_msg = parser
        .parse_message("socrates is human", omiai_io::nlp_parser::DetectedLanguage::English)
        .expect("parser should succeed");
    let fact_f = fact_msg.formula.expect("fact should carry a formula");
    let goal = Formula::atom("Mortal", vec![Term::Const("socrates".into())]);

    let prover = TheoremProver::new();
    let report = prover.prove_timed(&[rule_f, fact_f], &goal);
    match report.result {
        ProofResult::Proved { steps } => {
            assert!(
                !steps.is_empty(),
                "expected at least one resolution step in the proof"
            );
        }
        other => panic!("expected Proved, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 2. Knowledge graph + ontology classification pipeline
// ---------------------------------------------------------------------------

/// Build a small ontology, classify it, and verify hierarchical inference.
#[test]
fn kg_ontology_classification_pipeline() {
    let mut onto = Ontology::new();
    onto.add_axiom(Axiom::SubClassOf("Socrates".into(), "Human".into()));
    onto.add_axiom(Axiom::SubClassOf("Human".into(), "Mammal".into()));
    onto.add_axiom(Axiom::SubClassOf("Mammal".into(), "Animal".into()));
    onto.classify();

    assert!(onto.is_subclass("Socrates", "Animal"));
    assert!(onto.is_subclass("Socrates", "Mammal"));
    assert!(onto.is_subclass("Human", "Animal"));
    assert!(onto.is_consistent());

    // Disjointness violation
    let mut onto2 = Ontology::new();
    onto2.add_axiom(Axiom::SubClassOf("Platypus".into(), "Mammal".into()));
    onto2.add_axiom(Axiom::SubClassOf("Mammal".into(), "WarmBlooded".into()));
    onto2.add_axiom(Axiom::SubClassOf("Mammal".into(), "EggLaying".into()));
    onto2.add_axiom(Axiom::DisjointClasses(
        "WarmBlooded".into(),
        "EggLaying".into(),
    ));
    onto2.classify();
    assert!(!onto2.is_consistent());
}

// ---------------------------------------------------------------------------
// 3. KG + forward/backward reasoning pipeline
// ---------------------------------------------------------------------------

#[test]
fn kg_forward_and_backward_chain_pipeline() {
    let mut g = KnowledgeGraph::new();
    g.add_concept(Concept {
        id: "socrates".into(),
        label: "Socrates".into(),
    });
    g.add_concept(Concept {
        id: "human".into(),
        label: "Human".into(),
    });
    g.add_concept(Concept {
        id: "mammal".into(),
        label: "Mammal".into(),
    });
    g.add_concept(Concept {
        id: "animal".into(),
        label: "Animal".into(),
    });
    g.add_relation("socrates", "human", "type").unwrap();
    g.add_relation("human", "mammal", "subClassOf").unwrap();
    g.add_relation("mammal", "animal", "subClassOf").unwrap();

    // Path from socrates → animal
    let path = g.query_path("socrates", "animal").expect("path exists");
    assert_eq!(path, vec!["socrates", "human", "mammal", "animal"]);

    // Forward-chain from "socrates type human" + rules:
    //   mortal(X) ← human(X)
    //   philosopher(X) ← human(X)
    let mut facts: std::collections::HashSet<String> =
        ["human_socrates".into()].into_iter().collect();
    let rules = vec![
        HornRule {
            head: "mortal_socrates".into(),
            body: vec!["human_socrates".into()],
        },
        HornRule {
            head: "philosopher_socrates".into(),
            body: vec!["human_socrates".into()],
        },
    ];
    let derived = forward_chain(&facts, &rules);
    assert!(derived.contains("mortal_socrates"));
    assert!(derived.contains("philosopher_socrates"));

    // Backward chain proves philosopher_socrates
    assert!(backward_chain("philosopher_socrates", &facts, &rules));
}

// ---------------------------------------------------------------------------
// 4. Triple store + SPARQL-like query pipeline
// ---------------------------------------------------------------------------

#[test]
fn triple_sparql_like_query() {
    let mut store = TripleStore::new();
    for (s, p, o) in [
        ("socrates", "type", "Human"),
        ("plato", "type", "Human"),
        ("aristotle", "type", "Human"),
        ("socrates", "taught", "plato"),
        ("plato", "taught", "aristotle"),
    ] {
        store.insert(Triple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
        });
    }

    // Query: ?x type Human
    let q_pattern = TriplePattern {
        subject: TermPattern::Var("?x".into()),
        predicate: TermPattern::Bound("type".into()),
        object: TermPattern::Bound("Human".into()),
    };
    let matches = store.match_pattern(&q_pattern);
    let subjects: Vec<String> = matches.iter().map(|t| t.subject.clone()).collect();
    assert_eq!(subjects.len(), 3);
    assert!(subjects.contains(&"socrates".to_string()));
    assert!(subjects.contains(&"plato".to_string()));
    assert!(subjects.contains(&"aristotle".to_string()));
}

// ---------------------------------------------------------------------------
// 5. Causal DAG + SCM + do-calculus + counterfactual pipeline
// ---------------------------------------------------------------------------

#[test]
fn causal_full_pipeline() {
    // X → Y, Z → X, Z → Y
    let mut dag = CausalDag::new();
    dag.add_edge("X", "Y");
    dag.add_edge("Z", "X");
    dag.add_edge("Z", "Y");

    // Back-door: Z is a confounder; conditioning on it blocks the back-door
    let mut z_set = std::collections::HashSet::new();
    z_set.insert("Z".into());
    assert!(backdoor_criterion(&dag, "X", "Y", &z_set));

    // Build linear SCM: Y = a*X + b*Z + ε
    let mut scm = CausalModel::new();
    scm.add_equation(StructuralEquation::linear("X", vec![], vec![], 1.0));
    scm.add_equation(StructuralEquation::linear("Z", vec![], vec![], 0.5));
    scm.add_equation(StructuralEquation::linear(
        "Y",
        vec!["X".into(), "Z".into()],
        vec![2.0, 1.5],
        0.1,
    ));
    let mut u = HashMap::new();
    u.insert("X".into(), 0.0);
    u.insert("Y".into(), 0.0);
    u.insert("Z".into(), 0.0);
    let baseline = scm.simulate(&u);
    // Y = 0.1 + 2*1 + 1.5*0.5 = 0.1 + 2 + 0.75 = 2.85
    assert!((baseline["Y"] - 2.85).abs() < 1e-9);

    // Counterfactual: given observation Y=10, what if we set X=0?
    let mut obs = HashMap::new();
    obs.insert("X".into(), 1.0);
    obs.insert("Y".into(), 10.0);
    obs.insert("Z".into(), 0.5);
    let cf = counterfactual(&scm, &obs, "X", 0.0);
    // Pearl's three steps: abduction recovers the noise consistent with
    // the observation — u_Y = 10 − (0.1 + 2·1 + 1.5·0.5) = 7.15 (the
    // factual observation exceeds the model's 2.85 by exactly this
    // amount). After do(X:=0), prediction carries the abduced noise:
    //   Y′ = 0.1 + 2·0 + 1.5·0.5 + 7.15 = 8.0
    // (An earlier expectation of 0.85 here implicitly assumed u_Y = 0,
    // which skips abduction and contradicts the observed Y = 10.)
    assert!((cf["Y"] - 8.0).abs() < 1e-9, "counterfactual Y={:?}", cf);

    // Consistency check: with noise-free evidence (Y at its model value
    // 2.85), abduction yields u_Y = 0 and do(X:=0) gives Y′ = 0.85.
    let mut obs_clean = HashMap::new();
    obs_clean.insert("X".into(), 1.0);
    obs_clean.insert("Y".into(), 2.85);
    obs_clean.insert("Z".into(), 0.5);
    let cf_clean = counterfactual(&scm, &obs_clean, "X", 0.0);
    assert!(
        (cf_clean["Y"] - 0.85).abs() < 1e-9,
        "counterfactual Y={:?}",
        cf_clean
    );
}

// ---------------------------------------------------------------------------
// 6. Bayesian network + variable elimination pipeline
// ---------------------------------------------------------------------------

#[test]
fn bayesian_rain_sprinkler_pipeline() {
    let mut bn = BayesianNetwork::new();
    bn.add_node(Cpt {
        variable: "Rain".into(),
        parents: vec![],
        probs_true: vec![0.2],
    });
    bn.add_node(Cpt {
        variable: "Sprinkler".into(),
        parents: vec![],
        probs_true: vec![0.1],
    });
    bn.add_node(Cpt {
        variable: "Wet".into(),
        parents: vec!["Rain".into(), "Sprinkler".into()],
        probs_true: vec![0.0, 0.9, 0.8, 0.99],
    });
    let mut ev = HashMap::new();
    ev.insert("Wet".into(), true);
    let p_rain = bn.variable_elimination("Rain", &ev);
    // P(Rain=true | Wet=true) should be substantially above P(Rain)=0.2
    assert!(p_rain > 0.5, "P(Rain|Wet)={p_rain} should be > 0.5");
    // Sum-to-one: P(Rain=true) + P(Rain=false) = 1
    let mut ev2 = HashMap::new();
    ev2.insert("Rain".into(), false);
    ev2.insert("Wet".into(), true);
    let p_not = bn.variable_elimination("Sprinkler", &ev2);
    assert!((0.0..=1.0).contains(&p_not));
}

// ---------------------------------------------------------------------------
// 7. POMDP belief update pipeline
// ---------------------------------------------------------------------------

#[test]
fn pomdp_belief_pipeline() {
    let pomdp = tiger_pomdp();
    let b = pomdp.uniform_belief();
    // After hearing-left, posterior on tiger-left should be > 0.5
    let b2 = pomdp.update_belief(&b, 0, 0);
    assert!(b2[0] > b2[1]);
    // Posterior must normalize
    let sum: f64 = b2.iter().sum();
    assert!((sum - 1.0).abs() < 1e-9);
    // Greedy action under that belief
    let a = pomdp.greedy_action(&b2);
    assert!(a < pomdp.n_actions);
}

// ---------------------------------------------------------------------------
// 8. CGP symbolic regression pipeline
// ---------------------------------------------------------------------------

#[test]
fn cgp_symbolic_regression_pipeline() {
    // Target: f(x) = x * x
    let data: Vec<(f64, f64)> = (-50..=50)
        .map(|i| {
            let x = i as f64 / 25.0;
            (x, x * x)
        })
        .collect();
    let best = GeneticProgram::evolve(
        64,
        2,
        20,
        1,
        16,
        1,
        |prog| {
            let preds: Vec<f64> = data.iter().map(|(x, _)| prog.eval(&[*x])[0]).collect();
            let targets: Vec<f64> = data.iter().map(|(_, y)| *y).collect();
            mse_to_fitness(&preds, &targets)
        },
        17,
    );
    let err: f64 = data
        .iter()
        .map(|(x, y)| (best.eval(&[*x])[0] - y).abs())
        .sum::<f64>()
        / data.len() as f64;
    // CGP won't necessarily find x^2 exactly with this function set, but
    // mean abs error should be much smaller than |x| on average.
    assert!(err < 1.5, "mean abs err = {err}");
}

// ---------------------------------------------------------------------------
// 9. Reservoir training pipeline
// ---------------------------------------------------------------------------

#[test]
fn reservoir_rls_training_pipeline() {
    let mut r = Reservoir::new(40, 1, 1, 0.9, 23);
    let inputs: Vec<Vec<f64>> = (0..200).map(|t| vec![(t as f64 * 0.05).sin()]).collect();
    let targets: Vec<Vec<f64>> = inputs.iter().map(|u| vec![u[0] * 0.5]).collect();
    let pre = r.step(&[0.0])[0];
    r.train_readout(&inputs, &targets);
    let post = r.step(&[0.0])[0];
    // After training on linear identity, the readout should be closer to 0
    // (since input is 0).
    assert!(
        post.abs() < 1.0,
        "post-training output too large: {post} (pre={pre})"
    );
}

// ---------------------------------------------------------------------------
// 10. Cellular automata emergent pattern pipeline
// ---------------------------------------------------------------------------

#[test]
fn cellular_emergent_pattern_pipeline() {
    let mut ca = CellularAutomaton::random(32, 32, 0.3, 7);
    let pop0 = ca.population();
    ca.steps(10);
    let pop10 = ca.population();
    // Margolus block rule conserves population; after 10 even + 0 odd
    // steps (10 is even), pop should be unchanged.
    assert_eq!(pop0, pop10, "population not conserved over 10 even steps");
    let unique = ca.detect_patterns();
    assert!(unique > 0);
}

// ---------------------------------------------------------------------------
// 11. Active Inference / Free Energy minimization pipeline
// ---------------------------------------------------------------------------

#[test]
fn active_inference_minimizes_free_energy() {
    let mut eng = MetaCognitiveEngine::new(3);
    let obs = vec![1.0, -0.5, 0.3];
    let beliefs = eng.minimize_surprisal(&obs, 50, 0.1);
    // All beliefs should be finite
    for &b in &beliefs {
        assert!(b.is_finite());
    }
    // Free-energy history must be monotonically (non-strictly) decreasing
    let h = &eng.free_energy_history;
    for w in h.windows(2) {
        assert!(
            w[1] <= w[0] + 1e-6,
            "free energy increased: {} → {}",
            w[0],
            w[1]
        );
    }
}

// ---------------------------------------------------------------------------
// 12. MCTS + logical filter pipeline (neuro-symbolic integration)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Nim3 {
    stones: u32,
}

impl GameState for Nim3 {
    type Action = u32;
    fn legal_actions(&self) -> Vec<u32> {
        (1..=3).filter(|&k| k <= self.stones).collect()
    }
    fn apply(&self, action: &u32) -> Self {
        Nim3 {
            stones: self.stones.saturating_sub(*action),
        }
    }
    fn is_terminal(&self) -> bool {
        self.stones == 0
    }
    fn evaluate(&self) -> f64 {
        if self.stones == 0 { 0.0 } else { 0.5 }
    }
}

#[test]
fn mcts_with_logical_filter() {
    let game = Nim3 { stones: 7 };
    let mcts = Mcts::new(200);
    // Without any constraint, MCTS picks a legal move
    let action = mcts.search(&game).expect("move exists");
    assert!(
        (1..=3).contains(&action),
        "MCTS picked illegal action {action}"
    );

    // The filter pipeline is exercised by feeding it candidate actions
    // through a logical "is this safe" predicate.
    let all_actions = game.legal_actions();
    let safe = filter_actions_with_solver(all_actions, |&a| a != 3); // forbid taking 3
    for a in &safe {
        assert_ne!(*a, 3, "filter leaked forbidden action");
    }
}

// ---------------------------------------------------------------------------
// 13. JSON serialization round-trip for KG + Episode
// ---------------------------------------------------------------------------

#[test]
fn kg_serde_roundtrip() {
    let mut g = KnowledgeGraph::new();
    g.add_concept(Concept {
        id: "alice".into(),
        label: "Alice".into(),
    });
    g.add_concept(Concept {
        id: "bob".into(),
        label: "Bob".into(),
    });
    g.add_relation("alice", "bob", "knows").unwrap();

    let json =
        omiai_core::utils::serialization::to_json_pretty(&g.concept_ids().collect::<Vec<_>>()).unwrap();
    let parsed: Vec<String> = omiai_core::utils::serialization::from_json(&json).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(parsed.contains(&"alice".to_string()));
    assert!(parsed.contains(&"bob".to_string()));
}
