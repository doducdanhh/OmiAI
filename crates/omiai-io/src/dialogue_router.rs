//! Dialogue router: routes parsed intents to the appropriate reasoning pillar.
//!
//! This module connects all 8 pillars into a unified reasoning pipeline:
//! - omiai-core (logic/proof)
//! - omiai-knowledge (knowledge graph, forward/backward chaining)
//! - omiai-probabilistic (Bayesian inference)
//! - omiai-causal (do-calculus, counterfactuals)
//! - omiai-neuro (reservoir computing for dynamics/diversity)
//! - omiai-world (emergent language, agent societies)

use omiai_core::inference::ProofResult;
use omiai_core::logic_engine::{Formula, Term};
use omiai_causal::{dag::CausalDag, do_calculus};
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_knowledge::reasoning::HornRule;
use omiai_neuro::reservoir::Reservoir;
use omiai_probabilistic::bayesian::BayesianNetwork;
use omiai_world::world_loop::{World, WorldConfig};
use std::collections::{HashMap, HashSet};

/// Unified reasoning result from any pillar.
#[derive(Debug, Clone)]
pub enum ReasoningResult {
    /// Logical proof from core prover.
    LogicalProof {
        query: Formula,
        proof: ProofResult,
        premises_used: Vec<Formula>,
    },
    /// Probabilistic inference result.
    Probabilistic {
        query: String,
        probability: f64,
        method: ProbMethod,
        evidence: HashMap<String, bool>,
    },
    /// Causal explanation.
    Causal {
        query: CausalQuery,
        explanation: CausalExplanation,
    },
    /// Knowledge graph path/query result.
    KnowledgeGraph {
        query: KnowledgeQuery,
        result: KnowledgeResult,
    },
    /// No pillar could answer.
    NoAnswer,
}

/// Method used for probabilistic inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbMethod {
    Exact,
    MCMC,
}

/// Causal query types.
#[derive(Debug, Clone)]
pub enum CausalQuery {
    Why { cause: String, effect: String },
    WhatIf { intervention: String, outcome: String },
    Counterfactual { actual: String, hypothetical: String },
}

/// Causal explanation result.
#[derive(Debug, Clone)]
pub struct CausalExplanation {
    pub is_causal: bool,
    pub adjustment_set: Option<HashSet<String>>,
    pub method: String,
    pub details: String,
}

/// Knowledge graph query types.
#[derive(Debug, Clone)]
pub enum KnowledgeQuery {
    Path { from: String, to: String },
    Transitive { relation: String },
    ConsistencyCheck,
    Subgraph { concepts: Vec<String> },
}

/// Knowledge graph result.
#[derive(Debug, Clone)]
pub enum KnowledgeResult {
    Path(Vec<String>),
    TransitiveClosure(Vec<(String, String)>),
    Consistency(bool),
    Subgraph(Box<KnowledgeGraph>),
    NotFound,
}

/// Dialogue router configuration.
#[derive(Debug, Clone)]
pub struct DialogueRouterConfig {
    /// Enable probabilistic reasoning pillar.
    pub enable_probabilistic: bool,
    /// Enable causal reasoning pillar.
    pub enable_causal: bool,
    /// Enable knowledge graph pillar.
    pub enable_knowledge: bool,
    /// Enable neuro/reservoir pillar (for diversity).
    pub enable_neuro: bool,
    /// Enable world query pillar.
    pub enable_world: bool,
    /// Probabilistic query threshold (0.0-1.0) — queries with "probably", "likely" etc.
    pub prob_threshold: f64,
    /// Causal query keywords.
    pub causal_keywords: Vec<String>,
}

impl Default for DialogueRouterConfig {
    fn default() -> Self {
        Self {
            enable_probabilistic: true,
            enable_causal: true,
            enable_knowledge: true,
            enable_neuro: true,
            enable_world: true,
            prob_threshold: 0.5,
            causal_keywords: vec![
                "why".into(),
                "tại sao".into(),
                "what if".into(),
                "nếu".into(),
                "counterfactual".into(),
                "phản sự kiện".into(),
            ],
        }
    }
}

/// Main dialogue router struct.
#[derive(Debug)]
pub struct DialogueRouter {
    config: DialogueRouterConfig,
    /// Knowledge graph for symbolic knowledge.
    knowledge_graph: KnowledgeGraph,
    /// Bayesian network for probabilistic reasoning.
    bayesian_network: Option<BayesianNetwork>,
    /// Causal DAG for causal reasoning.
    causal_dag: Option<CausalDag>,
    /// Reservoir for diversity/dynamics.
    reservoir: Option<Reservoir>,
    /// World for emergent language queries.
    world: Option<Box<World>>,
    /// Horn rules for forward/backward chaining.
    horn_rules: Vec<HornRule>,
}

impl Default for DialogueRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogueRouter {
    /// Create a new dialogue router with default configuration.
    pub fn new() -> Self {
        let config = DialogueRouterConfig::default();
        Self::with_config(config)
    }

    /// Create a dialogue router with custom configuration.
    pub fn with_config(config: DialogueRouterConfig) -> Self {
        // Initialize reservoir with reasonable defaults if enabled
        let reservoir = if config.enable_neuro {
            Some(Reservoir::new(50, 10, 5, 0.95, 42))
        } else {
            None
        };

        // Initialize world if enabled (small config for chat queries)
        let world = if config.enable_world {
            Some(Box::new(World::new(
                WorldConfig {
                    width: 16,
                    height: 16,
                    n_initial_atoms: 3,
                    initial_resources: 0.05,
                },
                12345,
            )))
        } else {
            None
        };

        Self {
            config,
            knowledge_graph: KnowledgeGraph::new(),
            bayesian_network: None,
            causal_dag: None,
            reservoir,
            world,
            horn_rules: Vec::new(),
        }
    }

    /// Set/load a Bayesian network for probabilistic queries.
    pub fn set_bayesian_network(&mut self, bn: BayesianNetwork) {
        self.bayesian_network = Some(bn);
    }

    /// Set/load a world for world queries.
    pub fn set_world(&mut self, world: World) {
        self.world = Some(Box::new(world));
    }

    /// Set/load a knowledge graph for knowledge queries.
    pub fn set_knowledge_graph(&mut self, kg: KnowledgeGraph) {
        self.knowledge_graph = kg;
    }

    /// Add a Bayesian network for probabilistic queries.
    pub fn add_bayesian_network(&mut self, bn: BayesianNetwork) {
        self.bayesian_network = Some(bn);
    }

    /// Set/load a causal DAG for causal queries.
    pub fn set_causal_dag(&mut self, dag: CausalDag) {
        self.causal_dag = Some(dag);
    }

    /// Set/load a reservoir for diversity.
    pub fn set_reservoir(&mut self, reservoir: Reservoir) {
        self.reservoir = Some(reservoir);
    }

    /// Add a Horn rule for knowledge graph chaining.
    pub fn add_horn_rule(&mut self, rule: HornRule) {
        self.horn_rules.push(rule);
    }

    /// Add a concept to the knowledge graph.
    pub fn add_concept(&mut self, concept: Concept) -> bool {
        self.knowledge_graph.add_concept(concept)
    }

    /// Add a relation to the knowledge graph.
    pub fn add_relation(
        &mut self,
        from: &str,
        to: &str,
        kind: impl Into<String>,
    ) -> Result<(), omiai_knowledge::graph::GraphError> {
        self.knowledge_graph.add_relation(from, to, kind)
    }

    /// Route a parsed intent to the appropriate pillar and return a unified result.
    pub fn route(
        &mut self,
        intent: &crate::nlp_parser::ParseIntent,
        formula: Option<&Formula>,
        query: Option<&Formula>,
        memory_facts: &[Formula],
        query_type: crate::nlp_parser::QueryType,
    ) -> ReasoningResult {
        use crate::nlp_parser::ParseIntent;

        match intent {
            ParseIntent::Assert => {
                if let Some(f) = formula {
                    self.handle_assertion(f, memory_facts)
                } else {
                    ReasoningResult::NoAnswer
                }
            }
            ParseIntent::Ask => {
                if let Some(q) = query {
                    self.handle_question_with_type(q, memory_facts, query_type)
                } else {
                    ReasoningResult::NoAnswer
                }
            }
            ParseIntent::Explain => {
                if let Some(q) = query {
                    self.handle_explanation(q, memory_facts)
                } else {
                    ReasoningResult::NoAnswer
                }
            }
            _ => ReasoningResult::NoAnswer,
        }
    }

    /// Handle an assertion: store in memory, also add to knowledge graph if new.
    fn handle_assertion(&mut self, formula: &Formula, memory_facts: &[Formula]) -> ReasoningResult {
        // Try to prove it with core prover first
        let prover = omiai_core::prover::TheoremProver::new();
        let premises: Vec<Formula> = memory_facts.to_vec();
        let proof = prover.prove(&premises, formula);

        // Also add to knowledge graph if it's a simple atom
        if let Formula::Atom(pred, args) = formula {
            if args.len() == 1 {
                if let Term::Const(entity) = &args[0] {
                    let concept_id = format!("{}_{}", pred, entity);
                    let _ = self.knowledge_graph.add_concept(Concept {
                        id: concept_id.clone(),
                        label: format!("{} {}", entity, pred),
                    });
                }
            }
        }

        ReasoningResult::LogicalProof {
            query: formula.clone(),
            proof,
            premises_used: premises,
        }
    }

    /// Handle a question by trying multiple pillars in order.
    fn handle_question(
        &mut self,
        query: &Formula,
        memory_facts: &[Formula],
    ) -> ReasoningResult {
        self.handle_question_with_type(query, memory_facts, crate::nlp_parser::QueryType::Logical)
    }

    /// Handle a question with explicit query type for pillar prioritization.
    fn handle_question_with_type(
        &mut self,
        query: &Formula,
        memory_facts: &[Formula],
        query_type: crate::nlp_parser::QueryType,
    ) -> ReasoningResult {
        use crate::nlp_parser::QueryType;

        // 1. Try core logic prover first (always)
        let prover = omiai_core::prover::TheoremProver::new();
        let premises: Vec<Formula> = memory_facts.to_vec();
        let proof = prover.prove(&premises, query);

        if matches!(proof, omiai_core::inference::ProofResult::Proved { .. }) {
            return ReasoningResult::LogicalProof {
                query: query.clone(),
                proof,
                premises_used: premises,
            };
        }

        // 2. Prioritize based on query_type
        match query_type {
            QueryType::Probabilistic => {
                // Probabilistic first, then others
                if self.config.enable_probabilistic {
                    if let Some(prob_result) = self.try_probabilistic(query) {
                        return ReasoningResult::Probabilistic {
                            query: prob_result.0,
                            probability: prob_result.1,
                            method: prob_result.2,
                            evidence: prob_result.3,
                        };
                    }
                }
                if self.config.enable_causal {
                    if let Some(causal_result) = self.try_causal(query) {
                        return ReasoningResult::Causal {
                            query: causal_result.0,
                            explanation: causal_result.1,
                        };
                    }
                }
                if self.config.enable_knowledge {
                    if let Some(kg_result) = self.try_knowledge_graph(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: kg_result.0,
                            result: kg_result.1,
                        };
                    }
                }
                if self.config.enable_world {
                    if let Some(world_result) = self.try_world(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: world_result.0,
                            result: world_result.1,
                        };
                    }
                }
            }
            QueryType::Causal => {
                // Causal first
                if self.config.enable_causal {
                    if let Some(causal_result) = self.try_causal(query) {
                        return ReasoningResult::Causal {
                            query: causal_result.0,
                            explanation: causal_result.1,
                        };
                    }
                }
                if self.config.enable_probabilistic {
                    if let Some(prob_result) = self.try_probabilistic(query) {
                        return ReasoningResult::Probabilistic {
                            query: prob_result.0,
                            probability: prob_result.1,
                            method: prob_result.2,
                            evidence: prob_result.3,
                        };
                    }
                }
                if self.config.enable_knowledge {
                    if let Some(kg_result) = self.try_knowledge_graph(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: kg_result.0,
                            result: kg_result.1,
                        };
                    }
                }
                if self.config.enable_world {
                    if let Some(world_result) = self.try_world(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: world_result.0,
                            result: world_result.1,
                        };
                    }
                }
            }
            QueryType::World => {
                // World first
                if self.config.enable_world {
                    if let Some(world_result) = self.try_world(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: world_result.0,
                            result: world_result.1,
                        };
                    }
                }
                if self.config.enable_knowledge {
                    if let Some(kg_result) = self.try_knowledge_graph(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: kg_result.0,
                            result: kg_result.1,
                        };
                    }
                }
                if self.config.enable_probabilistic {
                    if let Some(prob_result) = self.try_probabilistic(query) {
                        return ReasoningResult::Probabilistic {
                            query: prob_result.0,
                            probability: prob_result.1,
                            method: prob_result.2,
                            evidence: prob_result.3,
                        };
                    }
                }
                if self.config.enable_causal {
                    if let Some(causal_result) = self.try_causal(query) {
                        return ReasoningResult::Causal {
                            query: causal_result.0,
                            explanation: causal_result.1,
                        };
                    }
                }
            }
            QueryType::KnowledgeGraph => {
                // Knowledge graph first
                if self.config.enable_knowledge {
                    if let Some(kg_result) = self.try_knowledge_graph(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: kg_result.0,
                            result: kg_result.1,
                        };
                    }
                }
                if self.config.enable_world {
                    if let Some(world_result) = self.try_world(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: world_result.0,
                            result: world_result.1,
                        };
                    }
                }
                if self.config.enable_probabilistic {
                    if let Some(prob_result) = self.try_probabilistic(query) {
                        return ReasoningResult::Probabilistic {
                            query: prob_result.0,
                            probability: prob_result.1,
                            method: prob_result.2,
                            evidence: prob_result.3,
                        };
                    }
                }
                if self.config.enable_causal {
                    if let Some(causal_result) = self.try_causal(query) {
                        return ReasoningResult::Causal {
                            query: causal_result.0,
                            explanation: causal_result.1,
                        };
                    }
                }
            }
            QueryType::Logical => {
                // Default order: knowledge -> probabilistic -> causal -> world
                if self.config.enable_knowledge {
                    if let Some(kg_result) = self.try_knowledge_graph(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: kg_result.0,
                            result: kg_result.1,
                        };
                    }
                }
                if self.config.enable_probabilistic {
                    if let Some(prob_result) = self.try_probabilistic(query) {
                        return ReasoningResult::Probabilistic {
                            query: prob_result.0,
                            probability: prob_result.1,
                            method: prob_result.2,
                            evidence: prob_result.3,
                        };
                    }
                }
                if self.config.enable_causal {
                    if let Some(causal_result) = self.try_causal(query) {
                        return ReasoningResult::Causal {
                            query: causal_result.0,
                            explanation: causal_result.1,
                        };
                    }
                }
                if self.config.enable_world {
                    if let Some(world_result) = self.try_world(query) {
                        return ReasoningResult::KnowledgeGraph {
                            query: world_result.0,
                            result: world_result.1,
                        };
                    }
                }
            }
        }

        // 6. Try negation proof (core prover on negated query)
        let negated = Formula::Not(Box::new(query.clone()));
        let negative_proof = prover.prove(&premises, &negated);
        if matches!(negative_proof, omiai_core::inference::ProofResult::Proved { .. }) {
            return ReasoningResult::LogicalProof {
                query: negated,
                proof: negative_proof,
                premises_used: premises,
            };
        }

        ReasoningResult::NoAnswer
    }

    /// Handle an explanation request (why/how).
    fn handle_explanation(
        &mut self,
        query: &Formula,
        memory_facts: &[Formula],
    ) -> ReasoningResult {
        // Try causal explanation first
        if self.config.enable_causal {
            if let Some(causal_result) = self.try_causal(query) {
                return ReasoningResult::Causal {
                    query: causal_result.0,
                    explanation: causal_result.1,
                };
            }
        }

        // Fall back to knowledge graph path explanation
        if self.config.enable_knowledge {
            if let Some(kg_result) = self.try_knowledge_graph(query) {
                return ReasoningResult::KnowledgeGraph {
                    query: kg_result.0,
                    result: kg_result.1,
                };
            }
        }

        // Fall back to core proof
        let prover = omiai_core::prover::TheoremProver::new();
        let premises: Vec<Formula> = memory_facts.to_vec();
        let proof = prover.prove(&premises, query);

        ReasoningResult::LogicalProof {
            query: query.clone(),
            proof,
            premises_used: premises,
        }
    }

    /// Try knowledge graph query.
    fn try_knowledge_graph(&self, query: &Formula) -> Option<(KnowledgeQuery, KnowledgeResult)> {
        // Extract entities from formula
        let entities = extract_entities(query);
        if entities.len() >= 2 {
            // Path query between two entities
            let from = &entities[0];
            let to = &entities[1];
            if let Some(path) = self.knowledge_graph.query_path(from, to) {
                return Some((
                    KnowledgeQuery::Path {
                        from: from.clone(),
                        to: to.clone(),
                    },
                    KnowledgeResult::Path(path),
                ));
            }
        }

        // Check for transitive closure query
        if let Formula::Atom(pred, _) = query {
            if let Some(_closure) = self
                .knowledge_graph
                .infer_transitive(pred)
                .into_iter()
                .next()
            {
                return Some((
                    KnowledgeQuery::Transitive {
                        relation: pred.clone(),
                    },
                    KnowledgeResult::TransitiveClosure(
                        self.knowledge_graph.infer_transitive(pred),
                    ),
                ));
            }
        }

        None
    }

    /// Try probabilistic query.
    fn try_probabilistic(&self, query: &Formula) -> Option<(String, f64, ProbMethod, HashMap<String, bool>)> {
        let bn = self.bayesian_network.as_ref()?;

        // Extract variable name from formula
        let var_name = extract_main_variable(query)?;
        let evidence = HashMap::new(); // No evidence from chat context yet

        // Check if we should use exact or MCMC
        let free_vars = count_free_variables(bn, &evidence, &var_name);
        let method = if free_vars > 16 {
            ProbMethod::MCMC
        } else {
            ProbMethod::Exact
        };

        let prob = match method {
            ProbMethod::Exact => bn.variable_elimination(&var_name, &evidence),
            ProbMethod::MCMC => bn.mcmc(&var_name, &evidence, 2000),
        };

        Some((var_name, prob, method, evidence))
    }

    /// Try causal query.
    fn try_causal(&self, query: &Formula) -> Option<(CausalQuery, CausalExplanation)> {
        let dag = self.causal_dag.as_ref()?;

        // Extract cause/effect from formula structure
        let (cause, effect) = extract_causal_variables(query)?;

        // Check back-door criterion
        let empty_z = HashSet::new();
        let is_causal = do_calculus::backdoor_criterion(dag, &cause, &effect, &empty_z);

        let adjustment_set = if is_causal {
            Some(empty_z)
        } else {
            None
        };

        Some((
            CausalQuery::Why {
                cause: cause.clone(),
                effect: effect.clone(),
            },
            CausalExplanation {
                is_causal,
                adjustment_set,
                method: "backdoor_criterion".into(),
                details: format!(
                    "Causal relationship {} → {}: {} (back-door criterion)",
                    cause,
                    effect,
                    if is_causal { "supported" } else { "not supported" }
                ),
            },
        ))
    }

    /// Try world query (emergent language, agent stats).
    fn try_world(&self, query: &Formula) -> Option<(KnowledgeQuery, KnowledgeResult)> {
        let world = self.world.as_ref()?;
        let _entities = extract_entities(query);
        let query_str = format!("{:?}", query).to_lowercase();

        // Agent count query
        if query_str.contains("agent") || query_str.contains("population") {
            return Some((
                KnowledgeQuery::Subgraph {
                    concepts: vec!["agents".into()],
                },
                KnowledgeResult::Subgraph(Box::new(world.knowledge.subgraph(&["agents".into()]))),
            ));
        }

        // Vocabulary/convention query
        if query_str.contains("vocab") || query_str.contains("convention") || query_str.contains("symbol") {
            let concepts: Vec<String> = world
                .knowledge
                .concept_ids()
                .filter(|id| id.starts_with("symbol_") || id.starts_with("convention_"))
                .map(|s| s.to_string())
                .collect();
            return Some((
                KnowledgeQuery::Subgraph { concepts: concepts.clone() },
                KnowledgeResult::Subgraph(Box::new(world.knowledge.subgraph(&concepts))),
            ));
        }

        None
    }

    /// Step the reservoir for diversity (call periodically).
    pub fn step_reservoir(&mut self, input: &[f64]) -> Option<Vec<f64>> {
        self.reservoir.as_mut().map(|r| r.step(input))
    }

    /// Step the world simulation (call periodically).
    pub fn step_world(&mut self) {
        if let Some(w) = &mut self.world {
            w.step();
        }
    }

    /// Get reservoir state for diversity in responses.
    pub fn reservoir_state(&self) -> Option<&[f64]> {
        self.reservoir.as_ref().map(|r| r.state())
    }

    /// Get world reference for read-only queries.
    pub fn world(&self) -> Option<&World> {
        self.world.as_deref()
    }

    /// Get knowledge graph reference.
    pub fn knowledge_graph(&self) -> &KnowledgeGraph {
        &self.knowledge_graph
    }
}

/// Extract entity names from a formula.
fn extract_entities(formula: &Formula) -> Vec<String> {
    let mut entities = Vec::new();
    collect_entities(formula, &mut entities);
    entities
}

fn collect_entities(formula: &Formula, out: &mut Vec<String>) {
    match formula {
        Formula::Atom(_pred, args) => {
            for arg in args {
                if let Term::Const(s) = arg {
                    out.push(s.clone());
                }
            }
        }
        Formula::Not(f) => collect_entities(f, out),
        Formula::And(a, b) | Formula::Or(a, b) | Formula::Implies(a, b) => {
            collect_entities(a, out);
            collect_entities(b, out);
        }
        Formula::ForAll(_, f) | Formula::Exists(_, f) => collect_entities(f, out),
        _ => {}
    }
}

/// Extract the main variable name from a formula for probabilistic queries.
fn extract_main_variable(formula: &Formula) -> Option<String> {
    match formula {
        Formula::Atom(pred, args) if !args.is_empty() => {
            if let Term::Var(_v) = &args[0] {
                Some(pred.clone())
            } else if let Term::Const(_c) = &args[0] {
                Some(pred.clone())
            } else {
                Some(pred.clone())
            }
        }
        Formula::Atom(pred, _) => Some(pred.clone()),
        _ => None,
    }
}

/// Count free variables in Bayesian network given evidence and query.
fn count_free_variables(
    bn: &BayesianNetwork,
    evidence: &HashMap<String, bool>,
    query: &str,
) -> usize {
    bn.nodes
        .iter()
        .filter(|n| !evidence.contains_key(&n.variable) && n.variable != query)
        .count()
}

/// Extract cause and effect variable names from a causal query formula.
fn extract_causal_variables(formula: &Formula) -> Option<(String, String)> {
    // Simple heuristic: look for Implies(A, B) where A=cause, B=effect
    match formula {
        Formula::Implies(a, b) => {
            let cause = extract_main_variable(a)?;
            let effect = extract_main_variable(b)?;
            Some((cause, effect))
        }
        Formula::Atom(pred, args) if args.len() == 2 => {
            // Binary predicate: pred(cause, effect)
            let cause = match &args[0] {
                Term::Const(c) | Term::Var(c) => c.clone(),
                _ => return None,
            };
            let effect = match &args[1] {
                Term::Const(c) | Term::Var(c) => c.clone(),
                _ => return None,
            };
            Some((cause, effect))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omiai_core::logic_engine::{Formula, Term};

    #[test]
    fn router_creation_works() {
        let router = DialogueRouter::new();
        assert!(router.config.enable_probabilistic);
        assert!(router.config.enable_causal);
        assert!(router.config.enable_knowledge);
    }

    #[test]
    fn router_handles_assertion() {
        let mut router = DialogueRouter::new();
        let formula = Formula::atom("Human", vec![Term::Const("Socrates".into())]);
        let result = router.route(&crate::nlp_parser::ParseIntent::Assert, Some(&formula), None, &[], crate::nlp_parser::QueryType::Logical);
        assert!(matches!(result, ReasoningResult::LogicalProof { .. }));
    }

    #[test]
    fn router_handles_question_with_proof() {
        let mut router = DialogueRouter::new();
        // Add fact: Human(Socrates)
        let fact = Formula::atom("Human", vec![Term::Const("Socrates".into())]);
        // Add rule: Human(x) -> Mortal(x)
        let rule = Formula::ForAll(
            "x".into(),
            Box::new(Formula::Implies(
                Box::new(Formula::atom("Human", vec![Term::Var("x".into())])),
                Box::new(Formula::atom("Mortal", vec![Term::Var("x".into())])),
            )),
        );
        let facts = vec![fact, rule];

        let query = Formula::atom("Mortal", vec![Term::Const("Socrates".into())]);
        let result = router.route(&crate::nlp_parser::ParseIntent::Ask, None, Some(&query), &facts, crate::nlp_parser::QueryType::Logical);

        assert!(matches!(result, ReasoningResult::LogicalProof { proof, .. } if matches!(proof, omiai_core::inference::ProofResult::Proved { .. })));
    }

    #[test]
    fn knowledge_graph_path_query() {
        let mut router = DialogueRouter::new();
        let _ = router.add_concept(Concept { id: "a".into(), label: "A".into() });
        let _ = router.add_concept(Concept { id: "b".into(), label: "B".into() });
        let _ = router.add_concept(Concept { id: "c".into(), label: "C".into() });
        router.add_relation("a", "b", "related").unwrap();
        router.add_relation("b", "c", "related").unwrap();

        let formula = Formula::atom("related", vec![Term::Const("a".into()), Term::Const("c".into())]);
        let result = router.try_knowledge_graph(&formula);

        assert!(result.is_some());
        if let Some((_, KnowledgeResult::Path(path))) = result {
            assert_eq!(path, vec!["a", "b", "c"]);
        } else {
            panic!("Expected path result");
        }
    }
}