//! Structural Causal Models: structural equations evaluated in
//! topological order given exogenous noise.

use std::collections::HashMap;

use super::dag::CausalDag;

/// Structural assignment: `variable := f(parents, noise)`.
///
/// For generality `f` is represented as a closed-form linear combination
/// plus noise: `X = bias + Σ w_i P_i + noise`.
#[derive(Debug, Clone)]
pub struct StructuralEquation {
    pub variable: String,
    pub causes: Vec<String>,
    pub weights: Vec<f64>,
    pub bias: f64,
}

impl StructuralEquation {
    pub fn linear(
        variable: impl Into<String>,
        causes: Vec<String>,
        weights: Vec<f64>,
        bias: f64,
    ) -> Self {
        Self {
            variable: variable.into(),
            causes,
            weights,
            bias,
        }
    }

    /// Evaluate given parent values and exogenous noise u.
    pub fn eval(&self, values: &HashMap<String, f64>, noise: f64) -> f64 {
        let mut y = self.bias + noise;
        for (c, w) in self.causes.iter().zip(self.weights.iter()) {
            y += w * values.get(c).copied().unwrap_or(0.0);
        }
        y
    }
}

/// A Structural Causal Model (SCM).
#[derive(Debug, Clone, Default)]
pub struct CausalModel {
    pub equations: Vec<StructuralEquation>,
    pub dag: CausalDag,
}

impl CausalModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_equation(&mut self, eq: StructuralEquation) {
        for c in &eq.causes {
            self.dag.add_edge(c.clone(), eq.variable.clone());
        }
        if eq.causes.is_empty() {
            self.dag.add_node(eq.variable.clone());
        }
        self.equations.push(eq);
    }

    /// Simulate by topologically-ordered structural equation evaluation.
    pub fn simulate(&self, exogenous_noise: &HashMap<String, f64>) -> HashMap<String, f64> {
        let order = self
            .dag
            .topological_order()
            .unwrap_or_else(|| self.equations.iter().map(|e| e.variable.clone()).collect());
        let eq_map: HashMap<&str, &StructuralEquation> = self
            .equations
            .iter()
            .map(|e| (e.variable.as_str(), e))
            .collect();
        let mut values = HashMap::new();
        for var in order {
            if let Some(eq) = eq_map.get(var.as_str()) {
                let u = exogenous_noise.get(&var).copied().unwrap_or(0.0);
                let v = eq.eval(&values, u);
                values.insert(var, v);
            } else {
                let u = exogenous_noise.get(&var).copied().unwrap_or(0.0);
                values.insert(var, u);
            }
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_chain_simulation() {
        let mut m = CausalModel::new();
        m.add_equation(StructuralEquation::linear("X", vec![], vec![], 0.0));
        m.add_equation(StructuralEquation::linear(
            "Y",
            vec!["X".into()],
            vec![2.0],
            1.0,
        ));
        let mut u = HashMap::new();
        u.insert("X".into(), 3.0);
        u.insert("Y".into(), 0.0);
        let v = m.simulate(&u);
        // Y = 1 + 2*X = 1 + 6 = 7
        assert!((v["Y"] - 7.0).abs() < 1e-9);
    }
}
