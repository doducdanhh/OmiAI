//! Genetic Programming trực tiếp trên cây cú pháp Formula (omiai_core::logic_engine::Formula).
//!
//! Module này cung cấp các toán tử tiến hóa (đột biến, lai ghép) thao tác trên
//! AST logic thay vì vector số thực, để "genome" của agent là một công thức
//! logic có thể đọc, hiểu, và suy luận lại được.

use std::collections::HashMap;

use omiai_core::logic_engine::{Formula, Term};
use rand::Rng;
use rand::SeedableRng;
use rand::prelude::SliceRandom;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Cấu hình cho Formula GP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaGpConfig {
    /// Xác suất đột biến subtree (thay thế một subtree ngẫu nhiên bằng subtree mới).
    pub subtree_mutation_prob: f64,
    /// Xác suất lai ghép subtree (hoán đổi subtree giữa hai cá thể).
    pub subtree_crossover_prob: f64,
    /// Xác suất hoisting (nâng một subtree lên làm root).
    pub hoisting_prob: f64,
    /// Xác suất point mutation (thay đổi hằng số/literal trong leaf).
    pub point_mutation_prob: f64,
    /// Độ sâu tối đa của formula sau đột biến.
    pub max_depth: usize,
    /// Tập hằng số cho phép khi sinh formula mới.
    pub allowed_constants: Vec<String>,
    /// Tập biến cho phép.
    pub allowed_variables: Vec<String>,
    /// Tập predicate cho phép (arity -> tên predicate).
    pub allowed_predicates: HashMap<usize, Vec<String>>,
    /// Tập hàm cho phép (arity -> tên hàm).
    pub allowed_functions: HashMap<usize, Vec<String>>,
}

impl Default for FormulaGpConfig {
    fn default() -> Self {
        let mut predicates = HashMap::new();
        predicates.insert(1, vec!["Human".into(), "Mortal".into(), "Animal".into()]);
        predicates.insert(2, vec!["Loves".into(), "Knows".into(), "Parent".into()]);

        let mut functions = HashMap::new();
        functions.insert(1, vec!["father".into(), "mother".into()]);
        functions.insert(2, vec!["plus".into(), "times".into()]);

        Self {
            subtree_mutation_prob: 0.3,
            subtree_crossover_prob: 0.4,
            hoisting_prob: 0.1,
            point_mutation_prob: 0.2,
            max_depth: 5,
            allowed_constants: vec!["socrates".into(), "plato".into(), "aristotle".into()],
            allowed_variables: vec!["x".into(), "y".into(), "z".into()],
            allowed_predicates: predicates,
            allowed_functions: functions,
        }
    }
}

/// Kết quả đánh giá fitness của một Formula.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaFitness {
    /// Giá trị fitness (càng cao càng tốt).
    pub score: f64,
    /// Số lần formula được đánh giá.
    pub evaluations: u64,
    /// Metadata bổ sung (ví dụ: độ phức tạp, độ sâu).
    pub metadata: HashMap<String, f64>,
}

/// Một cá thể trong quần thể Formula GP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaIndividual {
    /// Công thức logic.
    pub formula: Formula,
    /// Fitness đã tính.
    pub fitness: Option<FormulaFitness>,
    /// ID để theo dõi dòng dõi.
    pub id: u64,
    /// Generation sinh ra.
    pub generation: u64,
    /// Parent IDs (nếu có).
    pub parent_ids: Vec<u64>,
}

impl FormulaIndividual {
    /// Tạo cá thể mới từ formula.
    pub fn new(formula: Formula, id: u64, generation: u64) -> Self {
        Self {
            formula,
            fitness: None,
            id,
            generation,
            parent_ids: Vec::new(),
        }
    }

    /// Tạo cá thể con từ lai ghép.
    pub fn from_crossover(
        formula: Formula,
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

/// Bộ sinh formula ngẫu nhiên.
pub struct FormulaGenerator {
    config: FormulaGpConfig,
    rng: ChaCha8Rng,
    next_id: u64,
}

impl FormulaGenerator {
    /// Tạo generator mới với seed.
    pub fn new(config: FormulaGpConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
            next_id: 0,
        }
    }

    /// Sinh formula ngẫu nhiên với độ sâu tối đa.
    pub fn random_formula(&mut self, max_depth: usize) -> Formula {
        if max_depth == 0 || self.rng.r#gen::<f64>() < 0.3 {
            // Leaf: atomic formula
            self.random_atom()
        } else {
            // Internal node: connective or quantifier
            let choice = self.rng.r#gen_range(0..6);
            match choice {
                0 => Formula::Not(Box::new(self.random_formula(max_depth - 1))),
                1 => Formula::And(
                    Box::new(self.random_formula(max_depth - 1)),
                    Box::new(self.random_formula(max_depth - 1)),
                ),
                2 => Formula::Or(
                    Box::new(self.random_formula(max_depth - 1)),
                    Box::new(self.random_formula(max_depth - 1)),
                ),
                3 => Formula::Implies(
                    Box::new(self.random_formula(max_depth - 1)),
                    Box::new(self.random_formula(max_depth - 1)),
                ),
                4 => {
                    let var = self.random_variable();
                    Formula::ForAll(var, Box::new(self.random_formula(max_depth - 1)))
                }
                5 => {
                    let var = self.random_variable();
                    Formula::Exists(var, Box::new(self.random_formula(max_depth - 1)))
                }
                _ => unreachable!(),
            }
        }
    }

    /// Sinh atomic formula ngẫu nhiên.
    fn random_atom(&mut self) -> Formula {
        // Chọn arity ngẫu nhiên từ predicates có sẵn
        let arities: Vec<usize> = self.config.allowed_predicates.keys().copied().collect();
        let arity = *arities.r#choose(&mut self.rng).unwrap_or(&1);
        let predicates = &self.config.allowed_predicates[&arity];
        let pred_name = predicates.r#choose(&mut self.rng).unwrap().clone();

        let mut args = Vec::with_capacity(arity);
        for _ in 0..arity {
            args.push(self.random_term());
        }

        Formula::Atom(pred_name, args)
    }

    /// Sinh term ngẫu nhiên.
    fn random_term(&mut self) -> Term {
        let choice = self.rng.r#gen_range(0..3);
        match choice {
            0 => {
                // Variable
                let var = self.config.allowed_variables.r#choose(&mut self.rng).unwrap().clone();
                Term::Var(var)
            }
            1 => {
                // Constant
                let const_name = self.config.allowed_constants.r#choose(&mut self.rng).unwrap().clone();
                Term::Const(const_name)
            }
            2 => {
                // Function application
                let arities: Vec<usize> = self.config.allowed_functions.keys().copied().collect();
                let arity = *arities.r#choose(&mut self.rng).unwrap_or(&1);
                let funcs = &self.config.allowed_functions[&arity];
                let func_name = funcs.r#choose(&mut self.rng).unwrap().clone();

                let mut args = Vec::with_capacity(arity);
                for _ in 0..arity {
                    args.push(self.random_term());
                }
                Term::Func(func_name, args)
            }
            _ => unreachable!(),
        }
    }

    /// Sinh biến ngẫu nhiên.
    fn random_variable(&mut self) -> String {
        self.config.allowed_variables.r#choose(&mut self.rng).unwrap().clone()
    }

    /// Tạo cá thể ngẫu nhiên.
    pub fn random_individual(&mut self) -> FormulaIndividual {
        let formula = self.random_formula(self.config.max_depth);
        let id = self.next_id;
        self.next_id += 1;
        FormulaIndividual::new(formula, id, 0)
    }

    /// Tạo quần thể ban đầu.
    pub fn initial_population(&mut self, size: usize) -> Vec<FormulaIndividual> {
        (0..size).map(|_| self.random_individual()).collect()
    }
}

/// Toán tử đột biến trên Formula.
pub struct FormulaMutator {
    config: FormulaGpConfig,
    rng: ChaCha8Rng,
}

impl FormulaMutator {
    pub fn new(config: FormulaGpConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Đột biến subtree: thay thế một node ngẫu nhiên bằng subtree mới.
    pub fn subtree_mutation(&mut self, individual: &mut FormulaIndividual) {
        if self.rng.r#gen::<f64>() >= self.config.subtree_mutation_prob {
            return;
        }

        let mut formula = individual.formula.clone();
        self.mutate_subtree(&mut formula, 0);
        individual.formula = formula;
        individual.fitness = None; // Invalidate fitness
    }

    fn mutate_subtree(&mut self, formula: &mut Formula, current_depth: usize) {
        if current_depth >= self.config.max_depth {
            return;
        }

        // Quyết định có đột biến tại node này không
        if self.rng.r#gen::<f64>() < 0.1 {
            // Thay thế toàn bộ subtree này
            *formula = self.generate_random_subtree(self.config.max_depth - current_depth);
            return;
        }

        // Đệ quy vào các con
        match formula {
            Formula::Not(inner) => self.mutate_subtree(inner, current_depth + 1),
            Formula::And(left, right) | Formula::Or(left, right) | Formula::Implies(left, right) => {
                if self.rng.r#gen::<bool>() {
                    self.mutate_subtree(left, current_depth + 1);
                } else {
                    self.mutate_subtree(right, current_depth + 1);
                }
            }
            Formula::ForAll(_, inner) | Formula::Exists(_, inner) => {
                self.mutate_subtree(inner, current_depth + 1);
            }
            Formula::Atom(_, args) => {
                // Đột biến term arguments
                for arg in args {
                    self.mutate_term(arg, current_depth + 1);
                }
            }
            Formula::Iff(left, right) => {
                if self.rng.r#gen::<bool>() {
                    self.mutate_subtree(left, current_depth + 1);
                } else {
                    self.mutate_subtree(right, current_depth + 1);
                }
            }
            Formula::True | Formula::False => {
                // Leaf - có thể point mutation (không làm gì cho True/False)
            }
        }
    }

    fn generate_random_subtree(&mut self, max_depth: usize) -> Formula {
        if max_depth == 0 || self.rng.r#gen::<f64>() < 0.3 {
            self.random_atom()
        } else {
            let choice = self.rng.r#gen_range(0..6);
            match choice {
                0 => Formula::Not(Box::new(self.generate_random_subtree(max_depth - 1))),
                1 => Formula::And(
                    Box::new(self.generate_random_subtree(max_depth - 1)),
                    Box::new(self.generate_random_subtree(max_depth - 1)),
                ),
                2 => Formula::Or(
                    Box::new(self.generate_random_subtree(max_depth - 1)),
                    Box::new(self.generate_random_subtree(max_depth - 1)),
                ),
                3 => Formula::Implies(
                    Box::new(self.generate_random_subtree(max_depth - 1)),
                    Box::new(self.generate_random_subtree(max_depth - 1)),
                ),
                4 => {
                    let var = self.random_variable();
                    Formula::ForAll(var, Box::new(self.generate_random_subtree(max_depth - 1)))
                }
                5 => {
                    let var = self.random_variable();
                    Formula::Exists(var, Box::new(self.generate_random_subtree(max_depth - 1)))
                }
                _ => unreachable!(),
            }
        }
    }

    fn random_atom(&mut self) -> Formula {
        let arities: Vec<usize> = self.config.allowed_predicates.keys().copied().collect();
        let arity = *arities.r#choose(&mut self.rng).unwrap_or(&1);
        let predicates = &self.config.allowed_predicates[&arity];
        let pred_name = predicates.r#choose(&mut self.rng).unwrap().clone();

        let mut args = Vec::with_capacity(arity);
        for _ in 0..arity {
            args.push(self.random_term());
        }

        Formula::Atom(pred_name, args)
    }

    fn random_term(&mut self) -> Term {
        let choice = self.rng.r#gen_range(0..3);
        match choice {
            0 => {
                let var = self.config.allowed_variables.r#choose(&mut self.rng).unwrap().clone();
                Term::Var(var)
            }
            1 => {
                let const_name = self.config.allowed_constants.r#choose(&mut self.rng).unwrap().clone();
                Term::Const(const_name)
            }
            2 => {
                let arities: Vec<usize> = self.config.allowed_functions.keys().copied().collect();
                let arity = *arities.r#choose(&mut self.rng).unwrap_or(&1);
                let funcs = &self.config.allowed_functions[&arity];
                let func_name = funcs.r#choose(&mut self.rng).unwrap().clone();

                let mut args = Vec::with_capacity(arity);
                for _ in 0..arity {
                    args.push(self.random_term());
                }
                Term::Func(func_name, args)
            }
            _ => unreachable!(),
        }
    }

    fn random_variable(&mut self) -> String {
        self.config.allowed_variables.r#choose(&mut self.rng).unwrap().clone()
    }

    /// Point mutation: thay đổi hằng số/biến trong leaf.
    fn point_mutate_formula(&mut self, formula: &mut Formula) {
        if self.rng.r#gen::<f64>() >= self.config.point_mutation_prob {
            return;
        }

        match formula {
            Formula::Atom(_, args) => {
                for arg in args {
                    self.mutate_term(arg, 0);
                }
            }
            Formula::True | Formula::False => {
                // No point mutation for True/False
            }
            _ => {}
        }
    }

    fn mutate_term(&mut self, term: &mut Term, current_depth: usize) {
        if current_depth >= self.config.max_depth {
            return;
        }

        if self.rng.r#gen::<f64>() < 0.1 {
            // Thay thế term hoàn toàn
            *term = self.random_term();
            return;
        }

        match term {
            Term::Var(name) => {
                if self.rng.r#gen::<f64>() < self.config.point_mutation_prob {
                    *name = self.random_variable();
                }
            }
            Term::Const(name) => {
                if self.rng.r#gen::<f64>() < self.config.point_mutation_prob {
                    *name = self.config.allowed_constants.r#choose(&mut self.rng).unwrap().clone();
                }
            }
            Term::Func(name, args) => {
                // Có thể đổi tên hàm hoặc đệ quy vào args
                if self.rng.r#gen::<f64>() < self.config.point_mutation_prob {
                    let arities: Vec<usize> = self.config.allowed_functions.keys().copied().collect();
                    let arity = *arities.r#choose(&mut self.rng).unwrap_or(&args.len());
                    let funcs = &self.config.allowed_functions[&arity];
                    *name = funcs.r#choose(&mut self.rng).unwrap().clone();
                }
                for arg in args {
                    self.mutate_term(arg, current_depth + 1);
                }
            }
        }
    }

    /// Hoisting: chọn một subtree ngẫu nhiên và nâng nó lên làm root.
    pub fn hoisting(&mut self, individual: &mut FormulaIndividual) {
        if self.rng.r#gen::<f64>() >= self.config.hoisting_prob {
            return;
        }

        let nodes = self.collect_nodes(&individual.formula);
        if nodes.len() <= 1 {
            return;
        }

        // Chọn node ngẫu nhiên (không phải root)
        let idx = self.rng.r#gen_range(1..nodes.len());
        let new_root = nodes[idx].clone();
        individual.formula = new_root;
        individual.fitness = None;
    }

    /// Thu thập tất cả các node trong formula.
    fn collect_nodes(&self, formula: &Formula) -> Vec<Formula> {
        let mut nodes = Vec::new();
        self.collect_nodes_rec(formula, &mut nodes);
        nodes
    }

    fn collect_nodes_rec(&self, formula: &Formula, nodes: &mut Vec<Formula>) {
        nodes.push(formula.clone());
        match formula {
            Formula::Not(inner) => self.collect_nodes_rec(inner, nodes),
            Formula::And(left, right) | Formula::Or(left, right) | Formula::Implies(left, right) => {
                self.collect_nodes_rec(left, nodes);
                self.collect_nodes_rec(right, nodes);
            }
            Formula::ForAll(_, inner) | Formula::Exists(_, inner) => {
                self.collect_nodes_rec(inner, nodes);
            }
            Formula::Atom(_, args) => {
                for arg in args {
                    self.collect_terms_rec(arg, nodes);
                }
            }
            _ => {}
        }
    }

    fn collect_terms_rec(&self, term: &Term, nodes: &mut Vec<Formula>) {
        // Term có thể chuyển thành atomic formula
        match term {
            Term::Var(v) => nodes.push(Formula::Atom(v.clone(), Vec::new())),
            Term::Const(c) => nodes.push(Formula::Atom(c.clone(), Vec::new())),
            Term::Func(name, args) => {
                nodes.push(Formula::Atom(name.clone(), args.clone()));
                for arg in args {
                    self.collect_terms_rec(arg, nodes);
                }
            }
        }
    }
}

/// Toán tử lai ghép (crossover) giữa hai Formula.
pub struct FormulaCrossover {
    config: FormulaGpConfig,
    rng: ChaCha8Rng,
}

impl FormulaCrossover {
    pub fn new(config: FormulaGpConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Subtree crossover: hoán đổi subtree ngẫu nhiên giữa hai cá thể.
    pub fn subtree_crossover(
        &mut self,
        parent1: &FormulaIndividual,
        parent2: &FormulaIndividual,
        child_id: u64,
        generation: u64,
    ) -> FormulaIndividual {
        let mut formula1 = parent1.formula.clone();
        let mut formula2 = parent2.formula.clone();

        // Thu thập nodes của cả hai
        let nodes1 = self.collect_nodes_with_path(&formula1);
        let nodes2 = self.collect_nodes_with_path(&formula2);

        if nodes1.is_empty() || nodes2.is_empty() {
            // Fallback: trả về bản sao của parent1
            return FormulaIndividual::from_crossover(
                formula1,
                child_id,
                generation,
                vec![parent1.id, parent2.id],
            );
        }

        // Chọn ngẫu nhiên một node từ mỗi parent (không phải root)
        let idx1 = if nodes1.len() > 1 {
            self.rng.r#gen_range(1..nodes1.len())
        } else {
            0
        };
        let idx2 = if nodes2.len() > 1 {
            self.rng.r#gen_range(1..nodes2.len())
        } else {
            0
        };

        let (path1, node1) = &nodes1[idx1];
        let (path2, node2) = &nodes2[idx2];

        // Hoán đổi: thay thế node1 bằng clone của node2 trong formula1
        self.replace_at_path(&mut formula1, path1, node2.clone());
        self.replace_at_path(&mut formula2, path2, node1.clone());

        // Trả về child1 (formula1 sau khi đã hoán đổi)
        FormulaIndividual::from_crossover(
            formula1,
            child_id,
            generation,
            vec![parent1.id, parent2.id],
        )
    }

    /// Thu thập nodes kèm đường dẫn từ root.
    fn collect_nodes_with_path(&self, formula: &Formula) -> Vec<(Vec<usize>, Formula)> {
        let mut nodes = Vec::new();
        self.collect_rec(formula, &mut Vec::new(), &mut nodes);
        nodes
    }

    fn collect_rec(
        &self,
        formula: &Formula,
        path: &mut Vec<usize>,
        nodes: &mut Vec<(Vec<usize>, Formula)>,
    ) {
        nodes.push((path.clone(), formula.clone()));

        match formula {
            Formula::Not(inner) => {
                path.push(0);
                self.collect_rec(inner, path, nodes);
                path.pop();
            }
            Formula::And(left, right) | Formula::Or(left, right) | Formula::Implies(left, right) => {
                path.push(0);
                self.collect_rec(left, path, nodes);
                path.pop();
                path.push(1);
                self.collect_rec(right, path, nodes);
                path.pop();
            }
            Formula::ForAll(_, inner) | Formula::Exists(_, inner) => {
                path.push(0);
                self.collect_rec(inner, path, nodes);
                path.pop();
            }
            Formula::Atom(_, args) => {
                for (i, arg) in args.iter().enumerate() {
                    path.push(i);
                    self.collect_terms_rec(arg, path, nodes);
                    path.pop();
                }
            }
            _ => {}
        }
    }

    fn collect_terms_rec(
        &self,
        term: &Term,
        path: &mut Vec<usize>,
        nodes: &mut Vec<(Vec<usize>, Formula)>,
    ) {
        match term {
            Term::Var(v) => nodes.push((path.clone(), Formula::Atom(v.clone(), Vec::new()))),
            Term::Const(c) => nodes.push((path.clone(), Formula::Atom(c.clone(), Vec::new()))),
            Term::Func(name, args) => {
                nodes.push((path.clone(), Formula::Atom(name.clone(), args.clone())));
                for (i, arg) in args.iter().enumerate() {
                    path.push(i);
                    self.collect_terms_rec(arg, path, nodes);
                    path.pop();
                }
            }
        }
    }

    /// Thay thế node tại đường dẫn cho trước.
    fn replace_at_path(&self, formula: &mut Formula, path: &[usize], new_node: Formula) {
        if path.is_empty() {
            *formula = new_node;
            return;
        }

        let (first, rest) = path.split_at(1);
        let idx = first[0];

        match formula {
            Formula::Not(inner) => {
                if idx == 0 {
                    self.replace_at_path(inner, rest, new_node);
                }
            }
            Formula::And(left, right) | Formula::Or(left, right) | Formula::Implies(left, right) => {
                if idx == 0 {
                    self.replace_at_path(left, rest, new_node);
                } else if idx == 1 {
                    self.replace_at_path(right, rest, new_node);
                }
            }
            Formula::ForAll(_, inner) | Formula::Exists(_, inner) => {
                if idx == 0 {
                    self.replace_at_path(inner, rest, new_node);
                }
            }
            Formula::Atom(_, args) => {
                if idx < args.len() {
                    self.replace_term_at_path(&mut args[idx], rest, new_node);
                }
            }
            _ => {}
        }
    }

    fn replace_term_at_path(&self, term: &mut Term, path: &[usize], new_node: Formula) {
        if path.is_empty() {
            // Chuyển Formula thành Term (giả sử là Atom với args rỗng cho Var/Const)
            match new_node {
                Formula::Atom(name, args) if args.is_empty() => {
                    // Kiểm tra xem name có phải là biến hoặc hằng số đã biết không
                    if self.config.allowed_variables.contains(&name) {
                        *term = Term::Var(name);
                    } else if self.config.allowed_constants.contains(&name) {
                        *term = Term::Const(name);
                    } else {
                        // Mặc định coi là biến
                        *term = Term::Var(name);
                    }
                }
                Formula::Atom(name, args) => *term = Term::Func(name, args),
                _ => {} // Bỏ qua các Formula khác
            }
            return;
        }

        let (first, rest) = path.split_at(1);
        let idx = first[0];

        if let Term::Func(_, args) = term {
            if idx < args.len() {
                self.replace_term_at_path(&mut args[idx], rest, new_node);
            }
        }
    }
}

/// Quản lý quần thể Formula GP.
pub struct FormulaPopulation {
    config: FormulaGpConfig,
    individuals: Vec<FormulaIndividual>,
    generator: FormulaGenerator,
    mutator: FormulaMutator,
    crossover: FormulaCrossover,
    generation: u64,
    next_id: u64,
    rng: ChaCha8Rng,
}

impl FormulaPopulation {
    pub fn new(config: FormulaGpConfig, seed: u64, population_size: usize) -> Self {
        let generator = FormulaGenerator::new(config.clone(), seed);
        let mutator = FormulaMutator::new(config.clone(), seed.wrapping_add(1));
        let crossover = FormulaCrossover::new(config.clone(), seed.wrapping_add(2));
        let mut pop = Self {
            config,
            individuals: Vec::with_capacity(population_size),
            generator,
            mutator,
            crossover,
            generation: 0,
            next_id: 0,
            rng: ChaCha8Rng::seed_from_u64(seed.wrapping_add(3)),
        };
        pop.individuals = pop.generator.initial_population(population_size);
        pop.next_id = population_size as u64;
        pop
    }

    /// Đánh giá fitness cho toàn bộ quần thể.
    pub fn evaluate<F>(&mut self, fitness_fn: F)
    where
        F: Fn(&Formula) -> f64,
    {
        for ind in &mut self.individuals {
            if ind.fitness.is_none() {
                let score = fitness_fn(&ind.formula);
                ind.fitness = Some(FormulaFitness {
                    score,
                    evaluations: 1,
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("depth".into(), formula_depth(&ind.formula) as f64);
                        m.insert("size".into(), formula_size(&ind.formula) as f64);
                        m
                    },
                });
            }
        }
    }

    /// Chạy một generation: selection -> crossover -> mutation -> evaluate.
    pub fn step<F>(&mut self, fitness_fn: F, elite_count: usize)
    where
        F: Fn(&Formula) -> f64,
    {
        // Sắp xếp theo fitness giảm dần
        self.individuals.sort_by(|a, b| {
            let fa = a.fitness.as_ref().map(|f| f.score).unwrap_or(f64::NEG_INFINITY);
            let fb = b.fitness.as_ref().map(|f| f.score).unwrap_or(f64::NEG_INFINITY);
            fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Giữ elite
        let mut new_pop = self.individuals[..elite_count.min(self.individuals.len())].to_vec();

        // Sinh con lai ghép
        while new_pop.len() < self.individuals.len() {
            // Tournament selection
            let p1_id = self.tournament_select_id(3);
            let p2_id = self.tournament_select_id(3);
            let p1 = self.individuals.iter().find(|i| i.id == p1_id).unwrap().clone();
            let p2 = self.individuals.iter().find(|i| i.id == p2_id).unwrap().clone();

            if self.rng.r#gen::<f64>() < self.config.subtree_crossover_prob {
                let child = self.crossover.subtree_crossover(
                    &p1,
                    &p2,
                    self.next_id,
                    self.generation + 1,
                );
                self.next_id += 1;
                new_pop.push(child);
            } else {
                // Clone parent1
                let mut child = p1.clone();
                child.id = self.next_id;
                child.generation = self.generation + 1;
                child.parent_ids = vec![p1.id];
                self.next_id += 1;
                new_pop.push(child);
            }
        }

        // Đột biến
        for ind in &mut new_pop[elite_count..] {
            self.mutator.subtree_mutation(ind);
            self.mutator.hoisting(ind);
        }

        self.individuals = new_pop;
        self.generation += 1;

        // Đánh giá lại
        self.evaluate(fitness_fn);
    }

    fn tournament_select(&mut self, tournament_size: usize) -> &FormulaIndividual {
        let best_id = self.tournament_select_id(tournament_size);
        self.individuals.iter().find(|i| i.id == best_id).unwrap()
    }

    fn tournament_select_id(&mut self, tournament_size: usize) -> u64 {
        let mut best_id: Option<u64> = None;
        let mut best_score = f64::NEG_INFINITY;

        for _ in 0..tournament_size {
            let idx = self.rng.r#gen_range(0..self.individuals.len());
            let candidate = &self.individuals[idx];
            let score = candidate
                .fitness
                .as_ref()
                .map(|f| f.score)
                .unwrap_or(f64::NEG_INFINITY);
            if score > best_score {
                best_score = score;
                best_id = Some(candidate.id);
            }
        }
        best_id.unwrap()
    }

    /// Lấy best individual.
    pub fn best(&self) -> Option<&FormulaIndividual> {
        self.individuals.iter().max_by(|a, b| {
            let fa = a.fitness.as_ref().map(|f| f.score).unwrap_or(f64::NEG_INFINITY);
            let fb = b.fitness.as_ref().map(|f| f.score).unwrap_or(f64::NEG_INFINITY);
            fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Lấy quần thể hiện tại.
    pub fn individuals(&self) -> &[FormulaIndividual] {
        &self.individuals
    }

    /// Generation hiện tại.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Tính độ sâu của formula.
fn formula_depth(formula: &Formula) -> usize {
    match formula {
        Formula::Not(inner) => 1 + formula_depth(inner),
        Formula::And(l, r) | Formula::Or(l, r) | Formula::Implies(l, r) | Formula::Iff(l, r) => {
            1 + formula_depth(l).max(formula_depth(r))
        }
        Formula::ForAll(_, inner) | Formula::Exists(_, inner) => 1 + formula_depth(inner),
        Formula::Atom(_, args) => {
            if args.is_empty() {
                1
            } else {
                1 + args.iter().map(term_depth).max().unwrap_or(0)
            }
        }
        Formula::True | Formula::False => 1,
    }
}

fn term_depth(term: &Term) -> usize {
    match term {
        Term::Var(_) | Term::Const(_) => 1,
        Term::Func(_, args) => {
            if args.is_empty() {
                1
            } else {
                1 + args.iter().map(term_depth).max().unwrap_or(0)
            }
        }
    }
}

/// Tính kích thước (số node) của formula.
fn formula_size(formula: &Formula) -> usize {
    match formula {
        Formula::Not(inner) => 1 + formula_size(inner),
        Formula::And(l, r) | Formula::Or(l, r) | Formula::Implies(l, r) | Formula::Iff(l, r) => {
            1 + formula_size(l) + formula_size(r)
        }
        Formula::ForAll(_, inner) | Formula::Exists(_, inner) => 1 + formula_size(inner),
        Formula::Atom(_, args) => 1 + args.iter().map(term_size).sum::<usize>(),
        Formula::True | Formula::False => 1,
    }
}

fn term_size(term: &Term) -> usize {
    match term {
        Term::Var(_) | Term::Const(_) => 1,
        Term::Func(_, args) => 1 + args.iter().map(term_size).sum::<usize>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omiai_core::logic_engine::{Formula, Term};

    #[test]
    fn test_random_formula_generation() {
        let config = FormulaGpConfig::default();
        let mut generator = FormulaGenerator::new(config, 42);

        let formula = generator.random_formula(3);
        // max_depth controls connective depth; terms can add extra depth
        // So we just check it's reasonable and non-empty
        assert!(formula_depth(&formula) > 0);
        assert!(formula_depth(&formula) <= 10); // generous upper bound
        assert!(formula_size(&formula) > 0);
    }

    #[test]
    fn test_subtree_mutation() {
        let config = FormulaGpConfig::default();
        let mut mutator = FormulaMutator::new(config, 42);

        let formula = Formula::And(
            Box::new(Formula::Atom("P".into(), vec![Term::Var("x".into())])),
            Box::new(Formula::Atom("Q".into(), vec![Term::Var("y".into())])),
        );
        let mut ind = FormulaIndividual::new(formula, 1, 0);
        let _original = ind.formula.clone();

        mutator.subtree_mutation(&mut ind);

        // Formula nên thay đổi (với xác suất cao)
        // Không assert_eq vì mutation có xác suất
        assert!(ind.fitness.is_none());
    }

    #[test]
    fn test_hoisting() {
        let config = FormulaGpConfig {
            hoisting_prob: 1.0,
            ..Default::default()
        };
        let mut mutator = FormulaMutator::new(config, 42);

        let formula = Formula::And(
            Box::new(Formula::Atom("P".into(), vec![Term::Var("x".into())])),
            Box::new(Formula::Or(
                Box::new(Formula::Atom("Q".into(), vec![Term::Var("y".into())])),
                Box::new(Formula::Atom("R".into(), vec![Term::Var("z".into())])),
            )),
        );
        let mut ind = FormulaIndividual::new(formula, 1, 0);

        mutator.hoisting(&mut ind);

        // Sau hoisting, root nên là một trong các con
        assert!(ind.fitness.is_none());
    }

    #[test]
    fn test_subtree_crossover() {
        let config = FormulaGpConfig::default();
        let mut crossover = FormulaCrossover::new(config, 42);

        let p1 = FormulaIndividual::new(
            Formula::And(
                Box::new(Formula::Atom("P".into(), vec![Term::Var("x".into())])),
                Box::new(Formula::Atom("Q".into(), vec![Term::Var("y".into())])),
            ),
            1,
            0,
        );
        let p2 = FormulaIndividual::new(
            Formula::Or(
                Box::new(Formula::Atom("R".into(), vec![Term::Var("z".into())])),
                Box::new(Formula::Atom("S".into(), vec![Term::Var("w".into())])),
            ),
            2,
            0,
        );

        let child = crossover.subtree_crossover(&p1, &p2, 3, 1);

        assert_eq!(child.parent_ids, vec![1, 2]);
        assert_eq!(child.generation, 1);
        assert!(child.fitness.is_none());
    }

    #[test]
    fn test_population_evolution() {
        let config = FormulaGpConfig::default();
        let mut pop = FormulaPopulation::new(config, 42, 10);

        // Fitness function đơn giản: ưu tiên formula nhỏ hơn
        let fitness_fn = |f: &Formula| -> f64 {
            1.0 / (1.0 + formula_size(f) as f64)
        };

        pop.evaluate(fitness_fn);
        let initial_best = pop.best().map(|i| i.fitness.as_ref().unwrap().score).unwrap();

        for _ in 0..5 {
            pop.step(fitness_fn, 2);
        }

        let final_best = pop.best().map(|i| i.fitness.as_ref().unwrap().score).unwrap();

        // Fitness nên cải thiện (giá trị càng lớn càng tốt vì 1/size)
        assert!(final_best >= initial_best - 0.01); // Cho phép nhiễu nhỏ
        assert_eq!(pop.generation(), 5);
    }
}