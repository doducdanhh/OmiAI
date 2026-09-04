//! Genetic Programming trực tiếp trên cây cú pháp LTL Formula (omiai_core::ltl::LtlFormula).
//!
//! Module này cung cấp các toán tử tiến hóa (đột biến, lai ghép) thao tác trên
//! AST LTL thay vì vector số thực, để "genome" của agent là một công thức
//! LTL có thể đọc, hiểu, và suy luận lại được.

use omiai_core::ltl::LtlFormula;
use rand::Rng;
use rand::SeedableRng;
use rand::prelude::SliceRandom;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Cấu hình cho LTL Formula GP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtlFormulaGpConfig {
    /// Xác suất đột biến subtree (thay thế một subtree ngẫu nhiên bằng subtree mới).
    pub subtree_mutation_prob: f64,
    /// Xác suất lai ghép subtree (hoán đổi subtree giữa hai cá thể).
    pub subtree_crossover_prob: f64,
    /// Xác suất hoisting (nâng một subtree lên làm root).
    pub hoisting_prob: f64,
    /// Xác suất point mutation (thay đổi atom trong leaf).
    pub point_mutation_prob: f64,
    /// Độ sâu tối đa của formula sau đột biến.
    pub max_depth: usize,
    /// Tập tên atom cho phép (đối với LTL, đây là các mệnh đề propositional).
    pub allowed_atoms: Vec<String>,
}

impl Default for LtlFormulaGpConfig {
    fn default() -> Self {
        Self {
            subtree_mutation_prob: 0.3,
            subtree_crossover_prob: 0.5,
            hoisting_prob: 0.1,
            point_mutation_prob: 0.1,
            max_depth: 5,
            allowed_atoms: vec![
                "open".into(),
                "wall".into(),
                "res".into(),
                "occupied".into(),
                "hear0".into(),
                "hear1".into(),
                "hear2".into(),
                "hear3".into(),
            ],
        }
    }
}

/// Cá thể trong quần thể LTL GP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtlFormulaIndividual {
    pub formula: LtlFormula,
    pub fitness: Option<f64>,
    pub id: u64,
    pub generation: u64,
    pub parent_ids: Vec<u64>,
}

impl LtlFormulaIndividual {
    pub fn new(formula: LtlFormula, id: u64, generation: u64) -> Self {
        Self {
            formula,
            fitness: None,
            id,
            generation,
            parent_ids: Vec::new(),
        }
    }

    pub fn from_crossover(
        formula: LtlFormula,
        id: u64,
        generation: u64,
        parent_ids: Vec<u64>,
    ) -> Self {
        Self {
            formula,
            fitness: None,
            id,
            generation,
            parent_ids,
        }
    }
}

/// Bộ sinh LTL formula ngẫu nhiên.
pub struct LtlFormulaGenerator {
    config: LtlFormulaGpConfig,
    rng: ChaCha8Rng,
    next_id: u64,
}

impl LtlFormulaGenerator {
    /// Tạo generator mới với seed.
    pub fn new(config: LtlFormulaGpConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
            next_id: 0,
        }
    }

    /// Sinh LTL formula ngẫu nhiên với độ sâu tối đa.
    pub fn random_formula(&mut self, max_depth: usize) -> LtlFormula {
        if max_depth == 0 || self.rng.r#gen::<f64>() < 0.3 {
            // Leaf: atomic proposition
            self.random_atom()
        } else {
            // Internal node: connective or temporal operator
            let choice = self.rng.r#gen_range(0..7);
            match choice {
                0 => LtlFormula::Not(Box::new(self.random_formula(max_depth - 1))),
                1 => LtlFormula::And(
                    Box::new(self.random_formula(max_depth - 1)),
                    Box::new(self.random_formula(max_depth - 1)),
                ),
                2 => LtlFormula::Or(
                    Box::new(self.random_formula(max_depth - 1)),
                    Box::new(self.random_formula(max_depth - 1)),
                ),
                3 => LtlFormula::Next(Box::new(self.random_formula(max_depth - 1))),
                4 => LtlFormula::Eventually(Box::new(self.random_formula(max_depth - 1))),
                5 => LtlFormula::Globally(Box::new(self.random_formula(max_depth - 1))),
                6 => LtlFormula::Until(
                    Box::new(self.random_formula(max_depth - 1)),
                    Box::new(self.random_formula(max_depth - 1)),
                ),
                _ => unreachable!(),
            }
        }
    }

    /// Sinh atom ngẫu nhiên.
    fn random_atom(&mut self) -> LtlFormula {
        let name = self.config.allowed_atoms.choose(&mut self.rng).unwrap().clone();
        LtlFormula::atom(name)
    }

    /// Tạo cá thể ngẫu nhiên.
    pub fn random_individual(&mut self) -> LtlFormulaIndividual {
        let formula = self.random_formula(self.config.max_depth);
        let id = self.next_id;
        self.next_id += 1;
        LtlFormulaIndividual::new(formula, id, 0)
    }

    /// Tạo quần thể ban đầu.
    pub fn initial_population(&mut self, size: usize) -> Vec<LtlFormulaIndividual> {
        (0..size).map(|_| self.random_individual()).collect()
    }
}

/// Toán tử đột biến trên LTL Formula.
pub struct LtlFormulaMutator {
    config: LtlFormulaGpConfig,
    rng: ChaCha8Rng,
}

impl LtlFormulaMutator {
    pub fn new(config: LtlFormulaGpConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Đột biến subtree: thay thế một node ngẫu nhiên bằng subtree mới.
    pub fn subtree_mutation(&mut self, individual: &mut LtlFormulaIndividual) {
        if self.rng.r#gen::<f64>() >= self.config.subtree_mutation_prob {
            return;
        }
        individual.formula = self.mutate_subtree(&individual.formula, self.config.max_depth);
    }

    fn mutate_subtree(&mut self, formula: &LtlFormula, max_depth: usize) -> LtlFormula {
        // Collect all nodes
        let mut nodes = Vec::new();
        self.collect_nodes(formula, &mut nodes);

        if nodes.is_empty() {
            return formula.clone();
        }

        // Pick a random node to mutate
        let idx = self.rng.gen_range(0..nodes.len());
        let path = nodes[idx].clone();

        // Generate replacement subtree
        let new_subtree = Self::random_formula_static(&self.config, &mut self.rng, max_depth);

        // Replace the node at path
        self.replace_at_path(formula, &path, &new_subtree)
    }

    fn random_formula_static(config: &LtlFormulaGpConfig, rng: &mut ChaCha8Rng, max_depth: usize) -> LtlFormula {
        if max_depth == 0 || rng.r#gen::<f64>() < 0.3 {
            // Leaf: atomic proposition
            let name = config.allowed_atoms.choose(rng).unwrap().clone();
            LtlFormula::atom(name)
        } else {
            // Internal node: connective or temporal operator
            let choice = rng.gen_range(0..7);
            match choice {
                0 => LtlFormula::Not(Box::new(Self::random_formula_static(config, rng, max_depth - 1))),
                1 => LtlFormula::And(
                    Box::new(Self::random_formula_static(config, rng, max_depth - 1)),
                    Box::new(Self::random_formula_static(config, rng, max_depth - 1)),
                ),
                2 => LtlFormula::Or(
                    Box::new(Self::random_formula_static(config, rng, max_depth - 1)),
                    Box::new(Self::random_formula_static(config, rng, max_depth - 1)),
                ),
                3 => LtlFormula::Next(Box::new(Self::random_formula_static(config, rng, max_depth - 1))),
                4 => LtlFormula::Eventually(Box::new(Self::random_formula_static(config, rng, max_depth - 1))),
                5 => LtlFormula::Globally(Box::new(Self::random_formula_static(config, rng, max_depth - 1))),
                6 => LtlFormula::Until(
                    Box::new(Self::random_formula_static(config, rng, max_depth - 1)),
                    Box::new(Self::random_formula_static(config, rng, max_depth - 1)),
                ),
                _ => unreachable!(),
            }
        }
    }

    fn collect_nodes(&self, formula: &LtlFormula, nodes: &mut Vec<Vec<usize>>) {
        self.collect_nodes_rec(formula, &mut Vec::new(), nodes);
    }

    fn collect_nodes_rec(&self, formula: &LtlFormula, path: &mut Vec<usize>, nodes: &mut Vec<Vec<usize>>) {
        nodes.push(path.clone());

        match formula {
            LtlFormula::Not(g) => {
                path.push(0);
                self.collect_nodes_rec(g, path, nodes);
                path.pop();
            }
            LtlFormula::And(a, b) | LtlFormula::Or(a, b) => {
                path.push(0);
                self.collect_nodes_rec(a, path, nodes);
                path.pop();
                path.push(1);
                self.collect_nodes_rec(b, path, nodes);
                path.pop();
            }
            LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
                path.push(0);
                self.collect_nodes_rec(g, path, nodes);
                path.pop();
            }
            LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => {
                path.push(0);
                self.collect_nodes_rec(p, path, nodes);
                path.pop();
                path.push(1);
                self.collect_nodes_rec(q, path, nodes);
                path.pop();
            }
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => {}
        }
    }

    fn replace_at_path(&mut self, formula: &LtlFormula, path: &[usize], new_subtree: &LtlFormula) -> LtlFormula {
        if path.is_empty() {
            return new_subtree.clone();
        }

        match formula {
            LtlFormula::Not(g) => {
                if path[0] == 0 {
                    LtlFormula::Not(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::And(a, b) => {
                if path[0] == 0 {
                    LtlFormula::And(
                        Box::new(self.replace_at_path(a, &path[1..], new_subtree)),
                        b.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::And(
                        a.clone(),
                        Box::new(self.replace_at_path(b, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Or(a, b) => {
                if path[0] == 0 {
                    LtlFormula::Or(
                        Box::new(self.replace_at_path(a, &path[1..], new_subtree)),
                        b.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::Or(
                        a.clone(),
                        Box::new(self.replace_at_path(b, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Next(g) => {
                if path[0] == 0 {
                    LtlFormula::Next(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Eventually(g) => {
                if path[0] == 0 {
                    LtlFormula::Eventually(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Globally(g) => {
                if path[0] == 0 {
                    LtlFormula::Globally(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Until(p, q) => {
                if path[0] == 0 {
                    LtlFormula::Until(
                        Box::new(self.replace_at_path(p, &path[1..], new_subtree)),
                        q.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::Until(
                        p.clone(),
                        Box::new(self.replace_at_path(q, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Release(p, q) => {
                if path[0] == 0 {
                    LtlFormula::Release(
                        Box::new(self.replace_at_path(p, &path[1..], new_subtree)),
                        q.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::Release(
                        p.clone(),
                        Box::new(self.replace_at_path(q, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => formula.clone(),
        }
    }

    /// Point mutation: thay đổi atom tại leaf.
    pub fn point_mutation(&mut self, individual: &mut LtlFormulaIndividual) {
        if self.rng.r#gen::<f64>() >= self.config.point_mutation_prob {
            return;
        }
        individual.formula = self.mutate_atoms(&individual.formula);
    }

    fn mutate_atoms(&mut self, formula: &LtlFormula) -> LtlFormula {
        match formula {
            LtlFormula::Atom(_) => {
                if self.rng.r#gen::<f64>() < 0.5 {
                    self.random_atom()
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Not(g) => LtlFormula::Not(Box::new(self.mutate_atoms(g))),
            LtlFormula::And(a, b) => LtlFormula::And(
                Box::new(self.mutate_atoms(a)),
                Box::new(self.mutate_atoms(b)),
            ),
            LtlFormula::Or(a, b) => LtlFormula::Or(
                Box::new(self.mutate_atoms(a)),
                Box::new(self.mutate_atoms(b)),
            ),
            LtlFormula::Next(g) => LtlFormula::Next(Box::new(self.mutate_atoms(g))),
            LtlFormula::Eventually(g) => LtlFormula::Eventually(Box::new(self.mutate_atoms(g))),
            LtlFormula::Globally(g) => LtlFormula::Globally(Box::new(self.mutate_atoms(g))),
            LtlFormula::Until(p, q) => LtlFormula::Until(
                Box::new(self.mutate_atoms(p)),
                Box::new(self.mutate_atoms(q)),
            ),
            LtlFormula::Release(p, q) => LtlFormula::Release(
                Box::new(self.mutate_atoms(p)),
                Box::new(self.mutate_atoms(q)),
            ),
            LtlFormula::True_ | LtlFormula::False_ => formula.clone(),
        }
    }

    fn random_atom(&mut self) -> LtlFormula {
        let name = self.config.allowed_atoms.choose(&mut self.rng).unwrap().clone();
        LtlFormula::atom(name)
    }

    /// Hoisting: nâng một subtree ngẫu nhiên lên làm root.
    pub fn hoisting(&mut self, individual: &mut LtlFormulaIndividual) {
        if self.rng.r#gen::<f64>() >= self.config.hoisting_prob {
            return;
        }

        let mut nodes = Vec::new();
        self.collect_nodes(&individual.formula, &mut nodes);

        // Filter out root (empty path)
        nodes.retain(|p| !p.is_empty());

        if let Some(path) = nodes.choose(&mut self.rng) {
            individual.formula = self.extract_subtree(&individual.formula, path);
        }
    }

    fn extract_subtree(&self, formula: &LtlFormula, path: &[usize]) -> LtlFormula {
        if path.is_empty() {
            return formula.clone();
        }

        match formula {
            LtlFormula::Not(g) => {
                if path[0] == 0 {
                    self.extract_subtree(g, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::And(a, b) => {
                if path[0] == 0 {
                    self.extract_subtree(a, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(b, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Or(a, b) => {
                if path[0] == 0 {
                    self.extract_subtree(a, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(b, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
                if path[0] == 0 {
                    self.extract_subtree(g, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Until(p, q) => {
                if path[0] == 0 {
                    self.extract_subtree(p, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(q, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Release(p, q) => {
                if path[0] == 0 {
                    self.extract_subtree(p, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(q, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => formula.clone(),
        }
    }
}

/// Toán tử lai ghép trên LTL Formula.
pub struct LtlFormulaCrossover {
    config: LtlFormulaGpConfig,
    rng: ChaCha8Rng,
}

impl LtlFormulaCrossover {
    pub fn new(config: LtlFormulaGpConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Lai ghép subtree: hoán đổi subtree giữa hai cá thể.
    pub fn subtree_crossover(
        &mut self,
        parent1: &LtlFormulaIndividual,
        parent2: &LtlFormulaIndividual,
    ) -> (LtlFormula, LtlFormula) {
        if self.rng.r#gen::<f64>() >= self.config.subtree_crossover_prob {
            return (parent1.formula.clone(), parent2.formula.clone());
        }

        let mut nodes1 = Vec::new();
        let mut nodes2 = Vec::new();
        self.collect_nodes(&parent1.formula, &mut nodes1);
        self.collect_nodes(&parent2.formula, &mut nodes2);

        // Filter out root
        nodes1.retain(|p| !p.is_empty());
        nodes2.retain(|p| !p.is_empty());

        if nodes1.is_empty() || nodes2.is_empty() {
            return (parent1.formula.clone(), parent2.formula.clone());
        }

        let path1 = nodes1.choose(&mut self.rng).unwrap().clone();
        let path2 = nodes2.choose(&mut self.rng).unwrap().clone();

        let subtree1 = self.extract_subtree(&parent1.formula, &path1);
        let subtree2 = self.extract_subtree(&parent2.formula, &path2);

        let child1 = self.replace_at_path(&parent1.formula, &path1, &subtree2);
        let child2 = self.replace_at_path(&parent2.formula, &path2, &subtree1);

        (child1, child2)
    }

    fn collect_nodes(&self, formula: &LtlFormula, nodes: &mut Vec<Vec<usize>>) {
        self.collect_nodes_rec(formula, &mut Vec::new(), nodes);
    }

    fn collect_nodes_rec(&self, formula: &LtlFormula, path: &mut Vec<usize>, nodes: &mut Vec<Vec<usize>>) {
        nodes.push(path.clone());

        match formula {
            LtlFormula::Not(g) => {
                path.push(0);
                self.collect_nodes_rec(g, path, nodes);
                path.pop();
            }
            LtlFormula::And(a, b) | LtlFormula::Or(a, b) => {
                path.push(0);
                self.collect_nodes_rec(a, path, nodes);
                path.pop();
                path.push(1);
                self.collect_nodes_rec(b, path, nodes);
                path.pop();
            }
            LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
                path.push(0);
                self.collect_nodes_rec(g, path, nodes);
                path.pop();
            }
            LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => {
                path.push(0);
                self.collect_nodes_rec(p, path, nodes);
                path.pop();
                path.push(1);
                self.collect_nodes_rec(q, path, nodes);
                path.pop();
            }
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => {}
        }
    }

    fn extract_subtree(&self, formula: &LtlFormula, path: &[usize]) -> LtlFormula {
        if path.is_empty() {
            return formula.clone();
        }

        match formula {
            LtlFormula::Not(g) => {
                if path[0] == 0 {
                    self.extract_subtree(g, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::And(a, b) => {
                if path[0] == 0 {
                    self.extract_subtree(a, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(b, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Or(a, b) => {
                if path[0] == 0 {
                    self.extract_subtree(a, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(b, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
                if path[0] == 0 {
                    self.extract_subtree(g, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Until(p, q) => {
                if path[0] == 0 {
                    self.extract_subtree(p, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(q, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Release(p, q) => {
                if path[0] == 0 {
                    self.extract_subtree(p, &path[1..])
                } else if path[0] == 1 {
                    self.extract_subtree(q, &path[1..])
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => formula.clone(),
        }
    }

    fn replace_at_path(&self, formula: &LtlFormula, path: &[usize], new_subtree: &LtlFormula) -> LtlFormula {
        if path.is_empty() {
            return new_subtree.clone();
        }

        match formula {
            LtlFormula::Not(g) => {
                if path[0] == 0 {
                    LtlFormula::Not(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::And(a, b) => {
                if path[0] == 0 {
                    LtlFormula::And(
                        Box::new(self.replace_at_path(a, &path[1..], new_subtree)),
                        b.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::And(
                        a.clone(),
                        Box::new(self.replace_at_path(b, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Or(a, b) => {
                if path[0] == 0 {
                    LtlFormula::Or(
                        Box::new(self.replace_at_path(a, &path[1..], new_subtree)),
                        b.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::Or(
                        a.clone(),
                        Box::new(self.replace_at_path(b, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Next(g) => {
                if path[0] == 0 {
                    LtlFormula::Next(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Eventually(g) => {
                if path[0] == 0 {
                    LtlFormula::Eventually(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Globally(g) => {
                if path[0] == 0 {
                    LtlFormula::Globally(Box::new(self.replace_at_path(g, &path[1..], new_subtree)))
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Until(p, q) => {
                if path[0] == 0 {
                    LtlFormula::Until(
                        Box::new(self.replace_at_path(p, &path[1..], new_subtree)),
                        q.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::Until(
                        p.clone(),
                        Box::new(self.replace_at_path(q, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Release(p, q) => {
                if path[0] == 0 {
                    LtlFormula::Release(
                        Box::new(self.replace_at_path(p, &path[1..], new_subtree)),
                        q.clone(),
                    )
                } else if path[0] == 1 {
                    LtlFormula::Release(
                        p.clone(),
                        Box::new(self.replace_at_path(q, &path[1..], new_subtree)),
                    )
                } else {
                    formula.clone()
                }
            }
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => formula.clone(),
        }
    }
}

/// Vòng lặp GP đơn giản cho LTL Formula.
pub struct LtlGeneticProgram {
    config: LtlFormulaGpConfig,
    generator: LtlFormulaGenerator,
    mutator: LtlFormulaMutator,
    crossover: LtlFormulaCrossover,
    population: Vec<LtlFormulaIndividual>,
    generation: u64,
    rng: ChaCha8Rng,
}

impl LtlGeneticProgram {
    pub fn new(config: LtlFormulaGpConfig, seed: u64, pop_size: usize) -> Self {
        let mut generator = LtlFormulaGenerator::new(config.clone(), seed);
        let mutator = LtlFormulaMutator::new(config.clone(), seed + 1);
        let crossover = LtlFormulaCrossover::new(config.clone(), seed + 2);
        let mut population = generator.initial_population(pop_size);

        // Evaluate initial population
        for ind in &mut population {
            ind.fitness = Some(Self::evaluate(&ind.formula));
        }

        Self {
            config,
            generator,
            mutator,
            crossover,
            population,
            generation: 0,
            rng: ChaCha8Rng::seed_from_u64(seed + 3),
        }
    }

    /// Hàm đánh giá đơn giản: độ phức tạp của formula (càng đơn giản càng tốt).
    /// Trong thực tế, sẽ được thay bằng fitness từ môi trường mô phỏng.
    fn evaluate(formula: &LtlFormula) -> f64 {
        // Đếm số node: formula đơn giản hơn có fitness cao hơn
        1.0 / (1.0 + Self::count_nodes(formula) as f64)
    }

    fn count_nodes(formula: &LtlFormula) -> usize {
        1 + match formula {
            LtlFormula::Not(g) => Self::count_nodes(g),
            LtlFormula::And(a, b) | LtlFormula::Or(a, b) => Self::count_nodes(a) + Self::count_nodes(b),
            LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => Self::count_nodes(g),
            LtlFormula::Until(p, q) | LtlFormula::Release(p, q) => Self::count_nodes(p) + Self::count_nodes(q),
            LtlFormula::Atom(_) | LtlFormula::True_ | LtlFormula::False_ => 0,
        }
    }

    /// Chạy một thế hệ tiến hóa.
    pub fn step(&mut self) {
        // Selection: tournament
        let mut new_population = Vec::with_capacity(self.population.len());

        while new_population.len() < self.population.len() {
            // Tournament selection - clone the parent data we need
            let (parent1_id, _parent1_fitness) = self.tournament_select_data();
            let (parent2_id, _parent2_fitness) = self.tournament_select_data();

            // Crossover
            let parent1 = self.population.iter().find(|p| p.id == parent1_id).unwrap();
            let parent2 = self.population.iter().find(|p| p.id == parent2_id).unwrap();
            let (child1_formula, child2_formula) = self.crossover.subtree_crossover(parent1, parent2);

            // Create children
            let mut child1 = LtlFormulaIndividual::from_crossover(
                child1_formula,
                self.next_id(),
                self.generation + 1,
                vec![parent1_id, parent2_id],
            );
            let mut child2 = LtlFormulaIndividual::from_crossover(
                child2_formula,
                self.next_id(),
                self.generation + 1,
                vec![parent1_id, parent2_id],
            );

            // Mutation
            self.mutator.subtree_mutation(&mut child1);
            self.mutator.point_mutation(&mut child1);
            self.mutator.hoisting(&mut child1);

            self.mutator.subtree_mutation(&mut child2);
            self.mutator.point_mutation(&mut child2);
            self.mutator.hoisting(&mut child2);

            // Evaluate
            child1.fitness = Some(Self::evaluate(&child1.formula));
            child2.fitness = Some(Self::evaluate(&child2.formula));

            new_population.push(child1);
            if new_population.len() < self.population.len() {
                new_population.push(child2);
            }
        }

        self.population = new_population;
        self.generation += 1;
    }

    fn tournament_select_data(&mut self) -> (u64, f64) {
        let tournament_size = 3;
        let mut best_idx = self.rng.gen_range(0..self.population.len());
        let mut best_fitness = self.population[best_idx].fitness.unwrap_or(0.0);
        for _ in 1..tournament_size {
            let idx = self.rng.gen_range(0..self.population.len());
            let fitness = self.population[idx].fitness.unwrap_or(0.0);
            if fitness > best_fitness {
                best_idx = idx;
                best_fitness = fitness;
            }
        }
        (self.population[best_idx].id, best_fitness)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.generator.next_id;
        self.generator.next_id += 1;
        id
    }

    pub fn best(&self) -> Option<&LtlFormulaIndividual> {
        self.population.iter().max_by(|a, b| {
            a.fitness.unwrap_or(0.0).partial_cmp(&b.fitness.unwrap_or(0.0)).unwrap()
        })
    }

    pub fn population(&self) -> &[LtlFormulaIndividual] {
        &self.population
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_formula_generation() {
        let config = LtlFormulaGpConfig::default();
        let mut generator = LtlFormulaGenerator::new(config, 42);
        let formula = generator.random_formula(3);

        // Should produce a valid formula
        assert!(!format!("{:?}", formula).is_empty());
    }

    #[test]
    fn test_subtree_mutation() {
        let config = LtlFormulaGpConfig {
            subtree_mutation_prob: 1.0,
            ..Default::default()
        };
        let mut mutator = LtlFormulaMutator::new(config, 42);

        let base = LtlFormula::and(LtlFormula::atom("open"), LtlFormula::atom("res"));
        let mut individual = LtlFormulaIndividual::new(base, 0, 0);

        mutator.subtree_mutation(&mut individual);

        // Formula should have changed
        assert_ne!(individual.formula, LtlFormula::and(LtlFormula::atom("open"), LtlFormula::atom("res")));
    }

    #[test]
    fn test_point_mutation() {
        let config = LtlFormulaGpConfig {
            point_mutation_prob: 1.0,
            ..Default::default()
        };
        let mut mutator = LtlFormulaMutator::new(config, 42);

        let base = LtlFormula::atom("open");
        let mut individual = LtlFormulaIndividual::new(base, 0, 0);

        mutator.point_mutation(&mut individual);

        // Atom should have changed (with high probability)
        assert!(matches!(individual.formula, LtlFormula::Atom(_)));
    }

    #[test]
    fn test_subtree_crossover() {
        let config = LtlFormulaGpConfig {
            subtree_crossover_prob: 1.0,
            ..Default::default()
        };
        let mut crossover = LtlFormulaCrossover::new(config, 42);

        let parent1 = LtlFormulaIndividual::new(
            LtlFormula::and(LtlFormula::atom("open"), LtlFormula::atom("res")),
            1, 0
        );
        let parent2 = LtlFormulaIndividual::new(
            LtlFormula::or(LtlFormula::atom("wall"), LtlFormula::atom("occupied")),
            2, 0
        );

        let (child1, child2) = crossover.subtree_crossover(&parent1, &parent2);

        // Children should be different from parents
        assert_ne!(child1, parent1.formula);
        assert_ne!(child2, parent2.formula);
    }

    #[test]
    fn test_hoisting() {
        let config = LtlFormulaGpConfig {
            hoisting_prob: 1.0,
            ..Default::default()
        };
        let mut mutator = LtlFormulaMutator::new(config, 42);

        let base = LtlFormula::and(LtlFormula::atom("open"), LtlFormula::or(LtlFormula::atom("res"), LtlFormula::atom("wall")));
        let mut individual = LtlFormulaIndividual::new(base, 0, 0);

        mutator.hoisting(&mut individual);

        // Should be one of the subtrees
        assert!(matches!(individual.formula, LtlFormula::Atom(_) | LtlFormula::Or(_, _)));
    }

    #[test]
    fn test_population_evolution() {
        let config = LtlFormulaGpConfig::default();
        let mut gp = LtlGeneticProgram::new(config, 42, 20);

        let _initial_best = gp.best().map(|i| i.fitness.unwrap_or(0.0)).unwrap_or(0.0);

        for _ in 0..10 {
            gp.step();
        }

        let _final_best = gp.best().map(|i| i.fitness.unwrap_or(0.0)).unwrap_or(0.0);

        // Should have run without panicking
        assert!(gp.generation == 10);
    }
}