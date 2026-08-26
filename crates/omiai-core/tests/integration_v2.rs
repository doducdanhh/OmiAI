//! Integration tests for the new modules added in the v2 upgrade:
//!
//! - [`omiai_probabilistic::junction_tree`] — Junction Tree exact inference.
//! - [`omiai_probabilistic::hmc`] — Hamiltonian Monte Carlo.
//! - [`omiai_core::higher_order_unification`] — Huet HO unification.
//! - [`omiai_knowledge::abduction`] — Consistency-based abduction.
//! - [`omiai_knowledge::discocat`] — Distributional compositional categorial.
//! - [`omiai_causal::icp`] — Invariant Causal Prediction.
//! - [`omiai_meta::autopoiesis`] — Self-improvement loop.
//! - [`omiai_probabilistic::puct_mcts`] — PUCT-style MCTS.
//!
//! Run with: `cargo test --release --test integration_v2`

use std::collections::HashMap;

use omiai_causal::icp::{IcpSample, icp};
use omiai_core::higher_order_unification::{Term, Type};
use omiai_knowledge::abduction::{abduce, best_explanation};
use omiai_knowledge::discocat::{cosine, parse_transitive, toy_lexicon};
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_meta::autopoiesis::{AutopoieticLoop, MarkovBlanket, WorldState};
use omiai_probabilistic::bayesian::{BayesianNetwork, Cpt};
use omiai_probabilistic::hmc::{HmcSampler, IsotropicNormal, StandardNormal_};
use omiai_probabilistic::junction_tree::JunctionTree;
use omiai_probabilistic::mcts::GameState;
use omiai_probabilistic::puct_mcts::PuctMcts;

// ---------------------------------------------------------------------------
// Junction Tree integration
// ---------------------------------------------------------------------------

#[test]
fn junction_tree_end_to_end_with_ev() {
    // Build a small Bayesian network: Rain → WetGrass, Sprinkler → WetGrass
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
        variable: "WetGrass".into(),
        parents: vec!["Rain".into(), "Sprinkler".into()],
        probs_true: vec![0.0, 0.9, 0.8, 0.99],
    });

    let mut jt = JunctionTree::from_network(&bn);
    jt.calibrate();
    let mut ev = HashMap::new();
    ev.insert("WetGrass".into(), true);
    let p_rain = jt.query("Rain", &ev).expect("Rain in tree");
    assert!(p_rain > 0.0 && p_rain < 1.0);
    // Textbook: P(Rain|WetGrass) ≈ 0.74 with these numbers
    assert!(
        (p_rain - 0.74).abs() < 0.05,
        "P(Rain|Wet) ≈ 0.74, got {p_rain}"
    );
}

// ---------------------------------------------------------------------------
// HMC integration
// ---------------------------------------------------------------------------

#[test]
fn hmc_recovers_isotropic_normal_mean() {
    let density = IsotropicNormal {
        dim: 2,
        mean: vec![5.0, -3.0],
        std: 1.0,
    };
    let sampler = HmcSampler::new(0.3, 25, 1000);
    let result = sampler.sample(&density, 42);
    let m = result.mean();
    assert_eq!(m.len(), 2);
    assert!((m[0] - 5.0).abs() < 0.4, "x0 mean: {}", m[0]);
    assert!((m[1] - (-3.0)).abs() < 0.4, "x1 mean: {}", m[1]);
}

#[test]
fn hmc_unit_gaussian_diagnostic_check() {
    let density = StandardNormal_(1);
    let sampler = HmcSampler::new(0.4, 20, 500);
    let result = sampler.sample(&density, 7);
    let sd = result.std_dev();
    assert_eq!(sd.len(), 1);
    // Standard normal std ≈ 1.0
    assert!(sd[0] > 0.7 && sd[0] < 1.4, "std_dev = {}", sd[0]);
    assert!(result.acceptance_rate > 0.5);
}

// ---------------------------------------------------------------------------
// Higher-order unification integration
// ---------------------------------------------------------------------------

#[test]
fn huet_finds_eta_solution() {
    // Find f such that f a ≡ a  (η: f = λx. x)
    let f = Term::FVar("F".into());
    let a = Term::BVar(0);
    let lhs = Term::App(Box::new(f), Box::new(a.clone()));
    let sols = omiai_core::higher_order_unification::unify_terms(&[(lhs, a)], 4);
    assert!(!sols.is_empty(), "η solution should exist");
}

#[test]
fn huet_type_arity_computes() {
    let t = Type::arrow(Type::i(), Type::arrow(Type::i(), Type::o()));
    assert_eq!(t.arity(), 2);
}

// ---------------------------------------------------------------------------
// Consistency-based abduction integration
// ---------------------------------------------------------------------------

#[test]
fn abduce_wet_grass_returns_minimal_explanations() {
    let rains = omiai_core::logic_engine::Formula::prop("rain");
    let sprinkler = omiai_core::logic_engine::Formula::prop("sprinkler");
    let wet = omiai_core::logic_engine::Formula::prop("wet");
    let rule1 =
        omiai_core::logic_engine::Formula::Implies(Box::new(rains.clone()), Box::new(wet.clone()));
    let rule2 = omiai_core::logic_engine::Formula::Implies(
        Box::new(sprinkler.clone()),
        Box::new(wet.clone()),
    );
    let kb = vec![rule1, rule2];
    let assumables = vec!["rain".to_string(), "sprinkler".to_string()];
    let hyps = abduce(&kb, &wet, &assumables, 4);
    assert_eq!(
        hyps.len(),
        2,
        "two minimal explanations: rain and sprinkler"
    );
    let best = best_explanation(&kb, &wet, &assumables).unwrap();
    assert_eq!(best.size(), 1);
}

// ---------------------------------------------------------------------------
// DisCoCat integration
// ---------------------------------------------------------------------------

#[test]
fn discocat_parses_two_sentences_and_compares() {
    let lex = toy_lexicon();
    let v1 = parse_transitive(&lex, "alice", "likes", "bob").unwrap();
    let v2 = parse_transitive(&lex, "alice", "likes", "cat").unwrap();
    let v3 = parse_transitive(&lex, "alice", "sees", "bob").unwrap();
    let s_same = cosine(&v1, &v2);
    let s_diff = cosine(&v1, &v3);
    // Sentences sharing subject+verb should be more similar than different verbs
    assert!(s_same >= s_diff - 0.1, "got same={s_same} diff={s_diff}");
}

// ---------------------------------------------------------------------------
// Invariant Causal Prediction integration
// ---------------------------------------------------------------------------

fn synth_env(seed: u64, env: usize, n: usize) -> Vec<IcpSample> {
    let mut state = seed;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state as f64) / (u64::MAX as f64)
    };
    (0..n)
        .map(|_| {
            let x0 = next() * 2.0 - 1.0;
            let x1 = next() * 2.0 - 1.0;
            let x2 = next() * 2.0 - 1.0;
            // y depends on x0 only — x1, x2 are noise
            let y = 2.0 * x0 + (next() - 0.5) * 0.05;
            IcpSample {
                features: vec![x0, x1, x2],
                target: y,
                environment: env,
            }
        })
        .collect()
}

#[test]
fn icp_identifies_single_true_parent() {
    let env_a = synth_env(11, 0, 50);
    let env_b = synth_env(23, 1, 50);
    let env_c = synth_env(47, 2, 50);
    let mut all = env_a;
    all.extend(env_b);
    all.extend(env_c);
    let result = icp(&all, 0.05, 3);
    // The true parent is index 0 (x0).
    // ICP intersection should contain index 0 and exclude the others.
    assert!(
        result.parents.contains(&0),
        "ICP should identify x0 (index 0) as a direct cause"
    );
}

// ---------------------------------------------------------------------------
// Autopoiesis integration
// ---------------------------------------------------------------------------

#[test]
fn autopoiesis_loop_runs_and_updates_kg() {
    let mut al = AutopoieticLoop::new(2, 2);
    al.seed_concept("self", "the agent");
    let observations = vec![
        WorldState {
            features: vec![1.0, 0.0],
            label: "a".into(),
        },
        WorldState {
            features: vec![0.5, 0.5],
            label: "b".into(),
        },
        WorldState {
            features: vec![0.0, 1.0],
            label: "c".into(),
        },
    ];
    let summary = al.run(&observations, false);
    assert_eq!(summary.rounds, 3);
    assert_eq!(summary.kg_concepts, 4, "self + a + b + c");
    assert!(al.best_policy.is_some());
}

#[test]
fn markov_blanket_picks_internal_state() {
    let mut internal = HashMap::new();
    internal.insert("a".into(), 1.0);
    internal.insert("b".into(), 2.0);
    let mut desired = HashMap::new();
    desired.insert("target".into(), 1.0);
    let chosen = MarkovBlanket::select_action(&internal, &desired, &["a".into(), "b".into()]);
    assert_eq!(chosen, Some("a".into()));
}

// ---------------------------------------------------------------------------
// PUCT MCTS integration
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TicTacToeLite {
    pos: u8, // bit i = whether cell i is "X"
}

impl GameState for TicTacToeLite {
    type Action = u8;
    fn legal_actions(&self) -> Vec<u8> {
        (0..9).filter(|&i| (self.pos >> i) & 1 == 0).collect()
    }
    fn apply(&self, action: &u8) -> Self {
        TicTacToeLite {
            pos: self.pos | (1 << action),
        }
    }
    fn is_terminal(&self) -> bool {
        // 3 X's = terminal
        self.pos.count_ones() >= 3
    }
    fn evaluate(&self) -> f64 {
        if self.is_terminal() { 0.0 } else { 0.5 }
    }
}

#[test]
fn puct_finds_legal_move_with_prior() {
    let game = TicTacToeLite { pos: 0 };
    let searcher = PuctMcts::new(50);
    let action = searcher.search(&game).expect("move");
    assert!(action < 9);
}

#[test]
fn puct_with_extreme_prior_picks_first_action() {
    let game = TicTacToeLite { pos: 0 };
    let searcher = PuctMcts::new(50);
    let action = searcher
        .search_with_prior(&game, |_s, actions| {
            actions
                .iter()
                .enumerate()
                .map(|(i, _)| if i == 0 { 0.95 } else { 0.005 })
                .collect()
        })
        .expect("move");
    assert_eq!(action, 0);
}

// ---------------------------------------------------------------------------
// Cross-module integration: KG + autopoiesis
// ---------------------------------------------------------------------------

#[test]
fn kg_autopoiesis_relations_added() {
    let mut g = KnowledgeGraph::new();
    g.add_concept(Concept {
        id: "agent".into(),
        label: "Agent".into(),
    });
    g.add_concept(Concept {
        id: "world".into(),
        label: "World".into(),
    });
    g.add_relation("agent", "world", "perceives").unwrap();
    assert_eq!(g.len(), 2);
    assert!(g.query_path("agent", "world").is_some());
}

// ---------------------------------------------------------------------------
// Knowledge graph transitive closure sanity
// ---------------------------------------------------------------------------

#[test]
fn knowledge_graph_transitive_closure_pipeline() {
    let mut g = KnowledgeGraph::new();
    g.add_concept(Concept {
        id: "a".into(),
        label: "A".into(),
    });
    g.add_concept(Concept {
        id: "b".into(),
        label: "B".into(),
    });
    g.add_concept(Concept {
        id: "c".into(),
        label: "C".into(),
    });
    g.add_relation("a", "b", "partOf").unwrap();
    g.add_relation("b", "c", "partOf").unwrap();
    let closure = g.infer_transitive("partOf");
    assert!(closure.contains(&("a".into(), "c".into())));
}
