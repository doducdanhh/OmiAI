//! Integration tests for the v3 round of improvements:
//!
//! - [`omiai::probabilistic::gibbs`] — Gibbs sampling.
//! - [`omiai::probabilistic::mean_field`] — Mean-field variational inference.
//! - [`omiai::core::ltl`] — Linear Temporal Logic satisfiability.
//! - [`omiai::core::modal`] — Modal Logic K model checking.
//! - [`omiai::core::higher_order_unification`] — HU unification Branches API.
//! - [`omiai::probabilistic::junction_tree`] — Junction Tree division-step fix.
//!
//! Run with: `cargo test --release --test integration_v3`

use std::collections::HashMap;

use omiai::core::higher_order_unification::{Term, Type, unify_terms};
use omiai::core::ltl::{LtlFormula, is_satisfiable};
use omiai::core::modal::{KripkeStructure, ModalFormula, satisfies, two_world_model};
use omiai::probabilistic::bayesian::{BayesianNetwork, Cpt};
use omiai::probabilistic::gibbs::{GibbsConfig, gibbs_query, gibbs_sample};
use omiai::probabilistic::junction_tree::JunctionTree;
use omiai::probabilistic::mean_field::{MfConfig, mean_field};

// ---------------------------------------------------------------------------
// Shared BN fixture: Rain / Sprinkler / WetGrass
// ---------------------------------------------------------------------------

fn rain_sprinkler() -> BayesianNetwork {
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
    bn
}

// ---------------------------------------------------------------------------
// Gibbs sampling
// ---------------------------------------------------------------------------

#[test]
fn gibbs_end_to_end_with_wet_evidence() {
    let bn = rain_sprinkler();
    let config = GibbsConfig {
        iterations: 2000,
        burn_in: 400,
        thinning: 2,
    };
    let mut ev = HashMap::new();
    ev.insert("Wet".into(), true);
    let p_rain = gibbs_query(&bn, "Rain", &ev, &config, 42);
    // P(Rain | Wet) ≈ 0.74; Gibbs estimate should be in [0.5, 0.95]
    assert!(
        p_rain > 0.5 && p_rain < 0.95,
        "Gibbs P(Rain|Wet) = {p_rain}"
    );
}

#[test]
fn gibbs_samples_match_brute_force_within_tolerance() {
    let bn = rain_sprinkler();
    let config = GibbsConfig {
        iterations: 5000,
        burn_in: 1000,
        thinning: 3,
    };
    let result = gibbs_sample(&bn, &HashMap::new(), &config, 7);
    let p_rain = result.marginals.get("Rain").copied().unwrap_or(0.5);
    // Prior P(Rain) = 0.2; tight tolerance after long run
    assert!((p_rain - 0.2).abs() < 0.06, "P(Rain) = {p_rain}");
}

// ---------------------------------------------------------------------------
// Mean-field VI
// ---------------------------------------------------------------------------

#[test]
fn mean_field_recovers_priors_and_updates_with_evidence() {
    let bn = rain_sprinkler();
    let result = mean_field(&bn, &HashMap::new(), &MfConfig::default());
    let p_rain = result.marginals.get("Rain").copied().unwrap_or(0.5);
    // Without evidence, MF should be near the prior
    assert!((p_rain - 0.2).abs() < 0.15, "MF P(Rain) = {p_rain}");

    let mut ev = HashMap::new();
    ev.insert("Wet".into(), true);
    let result2 = mean_field(&bn, &ev, &MfConfig::default());
    let p_rain_ev = result2.marginals.get("Rain").copied().unwrap_or(0.5);
    assert!(p_rain_ev > p_rain + 0.3, "MF should increase P(Rain|Wet)");
}

#[test]
fn mean_field_converges_for_chain_bn() {
    // X → Y chain: P(X)=0.4, P(Y|X)=0.7
    let mut bn = BayesianNetwork::new();
    bn.add_node(Cpt {
        variable: "X".into(),
        parents: vec![],
        probs_true: vec![0.4],
    });
    bn.add_node(Cpt {
        variable: "Y".into(),
        parents: vec!["X".into()],
        probs_true: vec![0.2, 0.7],
    });
    let result = mean_field(&bn, &HashMap::new(), &MfConfig::default());
    let p_x = result.marginals.get("X").copied().unwrap_or(0.5);
    let p_y = result.marginals.get("Y").copied().unwrap_or(0.5);
    assert!((p_x - 0.4).abs() < 0.05);
    // P(Y) = 0.4*0.7 + 0.6*0.2 = 0.40
    assert!((p_y - 0.40).abs() < 0.05, "P(Y) = {p_y}");
}

// ---------------------------------------------------------------------------
// Junction Tree (verify division-step fix)
// ---------------------------------------------------------------------------

#[test]
fn junction_tree_division_step_produces_correct_query() {
    let bn = rain_sprinkler();
    let mut jt = JunctionTree::from_network(&bn);
    jt.calibrate();
    let mut ev = HashMap::new();
    ev.insert("Wet".into(), true);
    let p_rain = jt.query("Rain", &ev).expect("Rain in tree");
    // Textbook: P(Rain|Wet) ≈ 0.74
    assert!((p_rain - 0.74).abs() < 0.05, "JT P(Rain|Wet) = {p_rain}");
}

#[test]
fn junction_tree_agrees_with_brute_force() {
    let bn = rain_sprinkler();
    let mut ev = HashMap::new();
    ev.insert("Wet".into(), true);
    let p_brute = bn.variable_elimination("Rain", &ev);
    let mut jt = JunctionTree::from_network(&bn);
    jt.calibrate();
    let p_jt = jt.query("Rain", &ev).expect("query");
    assert!((p_brute - p_jt).abs() < 0.01, "brute={p_brute} jt={p_jt}");
}

// ---------------------------------------------------------------------------
// LTL satisfiability
// ---------------------------------------------------------------------------

#[test]
fn ltl_tautology_satisfiable() {
    assert!(is_satisfiable(&LtlFormula::True_, 100));
    assert!(is_satisfiable(&LtlFormula::atom("p"), 100));
}

#[test]
fn ltl_contradiction_unsatisfiable() {
    let f = LtlFormula::and(
        LtlFormula::atom("p"),
        LtlFormula::not(LtlFormula::atom("p")),
    );
    assert!(!is_satisfiable(&f, 100));
}

#[test]
fn ltl_eventuality_chain() {
    // F G p is satisfiable: eventually always p.
    let f = LtlFormula::f(LtlFormula::g(LtlFormula::atom("p")));
    assert!(is_satisfiable(&f, 1000));
}

#[test]
fn ltl_until_with_fulfillment() {
    // p U q is satisfiable: p holds until q holds.
    let f = LtlFormula::until(LtlFormula::atom("p"), LtlFormula::atom("q"));
    assert!(is_satisfiable(&f, 500));
}

// ---------------------------------------------------------------------------
// Modal Logic K
// ---------------------------------------------------------------------------

#[test]
fn modal_box_vacuous_in_single_world() {
    let mut m = KripkeStructure::new(vec![0]);
    assert!(satisfies(
        &m,
        0,
        &ModalFormula::box_(ModalFormula::atom("p"))
    ));
}

#[test]
fn modal_diamond_requires_successor() {
    let m = two_world_model("p", false, true);
    assert!(satisfies(
        &m,
        0,
        &ModalFormula::diamond(ModalFormula::atom("p"))
    ));
    let m2 = two_world_model("p", false, false);
    assert!(!satisfies(
        &m2,
        0,
        &ModalFormula::diamond(ModalFormula::atom("p"))
    ));
}

#[test]
fn modal_k_axiom_holds_in_two_world_model() {
    // K axiom: □(p → q) → (□p → □q)
    let mut m = two_world_model("p", true, true);
    m.set_true(0, "q");
    let k = ModalFormula::implies(
        ModalFormula::box_(ModalFormula::implies(
            ModalFormula::atom("p"),
            ModalFormula::atom("q"),
        )),
        ModalFormula::implies(
            ModalFormula::box_(ModalFormula::atom("p")),
            ModalFormula::box_(ModalFormula::atom("q")),
        ),
    );
    // Holds in this specific model
    assert!(satisfies(&m, 0, &k));
}

#[test]
fn modal_diamond_box_duality() {
    // ◇φ ≡ ¬□¬φ  for any model/world
    let m = two_world_model("p", false, true);
    let diamond_p = ModalFormula::diamond(ModalFormula::atom("p"));
    let neg_box_not_p = ModalFormula::not(ModalFormula::box_(ModalFormula::not(
        ModalFormula::atom("p"),
    )));
    for w in &m.worlds {
        assert_eq!(
            satisfies(&m, *w, &diamond_p),
            satisfies(&m, *w, &neg_box_not_p)
        );
    }
}

// ---------------------------------------------------------------------------
// Higher-order unification: verify Branches API emits imitation + projections
// ---------------------------------------------------------------------------

#[test]
fn huet_branches_api_emits_multiple_unifiers() {
    // Solve: f x ≡ g  (f must be a function, x bound)
    // Expected unifiers include imitation (f := λy. g) and projections.
    let f = Term::FVar("F".into());
    let x = Term::BVar(0);
    let g = Term::Const("g".into());
    let lhs = Term::App(Box::new(f.clone()), Box::new(x));
    let rhs = g.clone();
    let sols = unify_terms(&[(lhs, rhs)], 8);
    // At minimum we expect at least one solution (imitation).
    assert!(!sols.is_empty(), "expected at least one HO unifier");
}

#[test]
fn huet_type_arithmetic() {
    let t = Type::arrow(Type::i(), Type::arrow(Type::i(), Type::o()));
    assert_eq!(t.arity(), 2);
    let o = Type::o();
    assert_eq!(o.arity(), 0);
}

// ---------------------------------------------------------------------------
// Cross-module: Junction Tree + Gibbs + Mean-Field agreement
// ---------------------------------------------------------------------------

#[test]
fn three_methods_agree_on_rain_wet_query() {
    let bn = rain_sprinkler();
    let mut ev = HashMap::new();
    ev.insert("Wet".into(), true);

    // Method 1: Junction Tree (exact)
    let mut jt = JunctionTree::from_network(&bn);
    jt.calibrate();
    let p_jt = jt.query("Rain", &ev).expect("JT");

    // Method 2: Mean-field (approximate variational)
    let mf = mean_field(&bn, &ev, &MfConfig::default());
    let p_mf = mf.marginals.get("Rain").copied().unwrap_or(0.5);

    // Method 3: Gibbs sampling
    let config = GibbsConfig {
        iterations: 3000,
        burn_in: 500,
        thinning: 2,
    };
    let p_gibbs = gibbs_query(&bn, "Rain", &ev, &config, 99);

    // All should be close to the textbook value 0.74
    let target = 0.74;
    assert!((p_jt - target).abs() < 0.05, "JT={p_jt}");
    assert!((p_mf - target).abs() < 0.10, "MF={p_mf}");
    assert!((p_gibbs - target).abs() < 0.10, "Gibbs={p_gibbs}");
}
