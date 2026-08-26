//! Interventions and counterfactuals on SCMs (abduction–action–prediction).

use std::collections::HashMap;

use super::dag::CausalDag;
use super::do_calculus::remove_incoming;
use super::scm::{CausalModel, StructuralEquation};

/// Graph surgery: cut all incoming edges to the intervened variable(s)
/// and clamp their values (do-operator).
pub fn intervene_graph(dag: &CausalDag, variables: &[&str]) -> CausalDag {
    let mut g = dag.clone();
    for &v in variables {
        g = remove_incoming(&g, v);
    }
    g
}

/// Soft/hard intervention on an SCM: replace structural equation for `var`
/// with the constant `value` (atomic intervention do(Var := value)).
pub fn intervene_model(model: &CausalModel, var: &str, value: f64) -> CausalModel {
    let mut out = CausalModel::new();
    out.dag = remove_incoming(&model.dag, var);
    for eq in &model.equations {
        if eq.variable == var {
            out.add_equation(StructuralEquation::linear(var, vec![], vec![], value));
        } else {
            out.equations.push(eq.clone());
            // rebuild edges for non-intervened
            for c in &eq.causes {
                if eq.variable != var {
                    out.dag.add_edge(c.clone(), eq.variable.clone());
                }
            }
        }
    }
    // Ensure intervened node present
    if !out.equations.iter().any(|e| e.variable == var) {
        out.add_equation(StructuralEquation::linear(var, vec![], vec![], value));
    }
    out
}

/// Counterfactual: given evidence (observed world), intervene, predict.
///
/// Three-step procedure (Pearl):
/// 1. **Abduction** — invert noise from observations under the factual SCM
/// 2. **Action** — apply intervention
/// 3. **Prediction** — simulate the modified SCM with abduced noise
pub fn counterfactual(
    model: &CausalModel,
    evidence: &HashMap<String, f64>,
    intervene_var: &str,
    intervene_val: f64,
) -> HashMap<String, f64> {
    // Abduction for linear SCMs: recover noise u_i = x_i - (bias + w·parents)
    let order = model
        .dag
        .topological_order()
        .unwrap_or_else(|| model.equations.iter().map(|e| e.variable.clone()).collect());
    let eq_map: HashMap<&str, &StructuralEquation> = model
        .equations
        .iter()
        .map(|e| (e.variable.as_str(), e))
        .collect();
    let mut noise = HashMap::new();
    let mut factual = HashMap::new();
    for var in &order {
        if let Some(eq) = eq_map.get(var.as_str()) {
            let observed = evidence.get(var).copied().unwrap_or_else(|| {
                // if not observed, use 0 noise simulation
                eq.eval(&factual, 0.0)
            });
            let predicted_zero_noise = eq.eval(&factual, 0.0);
            let u = observed - predicted_zero_noise;
            noise.insert(var.clone(), u);
            factual.insert(var.clone(), observed);
        }
    }

    let modified = intervene_model(model, intervene_var, intervene_val);
    // Under intervention, noise for intervened var is irrelevant (constant eq)
    noise.insert(intervene_var.to_string(), 0.0);
    modified.simulate(&noise)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causal::scm::StructuralEquation;

    #[test]
    fn intervention_breaks_incoming() {
        let mut m = CausalModel::new();
        m.add_equation(StructuralEquation::linear("X", vec![], vec![], 0.0));
        m.add_equation(StructuralEquation::linear(
            "Y",
            vec!["X".into()],
            vec![2.0],
            0.0,
        ));
        let m2 = intervene_model(&m, "Y", 5.0);
        let mut u = HashMap::new();
        u.insert("X".into(), 10.0);
        u.insert("Y".into(), 0.0);
        let v = m2.simulate(&u);
        assert!((v["Y"] - 5.0).abs() < 1e-9);
    }
}
