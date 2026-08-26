//! Guarded continual learning for symbolic knowledge.
//!
//! New observations enter episodic memory first. Only repeated, sufficiently
//! confident and logically consistent candidates are promoted to durable
//! semantic facts. Contradictions are quarantined instead of silently replacing
//! established knowledge.

use serde::{Deserialize, Serialize};

use crate::core::inference::ProofResult;
use crate::core::logic_engine::Formula;
use crate::core::prover::TheoremProver;
use crate::memory::episodic::{Episode, EpisodicMemory};

/// Policy controlling promotion from observations to trusted knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPolicy {
    pub minimum_observations: usize,
    pub minimum_confidence: f64,
    pub maximum_facts: usize,
}

impl Default for LearningPolicy {
    fn default() -> Self {
        Self {
            minimum_observations: 2,
            minimum_confidence: 0.70,
            maximum_facts: 100_000,
        }
    }
}

/// Candidate rejected or delayed by a safety gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantinedKnowledge {
    pub formula: Formula,
    pub reason: String,
    pub confidence: f64,
}

/// Summary of one consolidation pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsolidationReport {
    pub promoted: usize,
    pub quarantined: usize,
    pub already_known: usize,
}

/// Long-lived symbolic learning state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinualLearningEngine {
    pub policy: LearningPolicy,
    pub episodic: EpisodicMemory,
    pub facts: Vec<Formula>,
    pub quarantine: Vec<QuarantinedKnowledge>,
    #[serde(skip, default)]
    prover: TheoremProver,
}

impl Default for ContinualLearningEngine {
    fn default() -> Self {
        Self::new(LearningPolicy::default())
    }
}

impl ContinualLearningEngine {
    /// Create a guarded learner with explicit promotion thresholds.
    pub fn new(policy: LearningPolicy) -> Self {
        Self {
            policy,
            episodic: EpisodicMemory::new(),
            facts: Vec::new(),
            quarantine: Vec::new(),
            prover: TheoremProver::new(),
        }
    }

    /// Record an observation without prematurely treating it as truth.
    pub fn observe(&mut self, episode: Episode) {
        self.episodic.remember(episode);
    }

    /// Promote repeated observations that do not contradict trusted facts.
    pub fn consolidate(&mut self) -> ConsolidationReport {
        let candidates = self.episodic.consolidation_candidates(
            self.policy.minimum_observations,
            self.policy.minimum_confidence,
        );
        let mut report = ConsolidationReport::default();

        for (formula, _, confidence) in candidates {
            if self.facts.contains(&formula) {
                report.already_known += 1;
                continue;
            }
            if self.facts.len() >= self.policy.maximum_facts {
                self.quarantine(formula, "knowledge capacity reached", confidence);
                report.quarantined += 1;
                continue;
            }
            let negated = Formula::Not(Box::new(formula.clone()));
            if matches!(
                self.prover.prove(&self.facts, &negated),
                ProofResult::Proved { .. }
            ) {
                self.quarantine(formula, "contradicts established knowledge", confidence);
                report.quarantined += 1;
                continue;
            }
            self.facts.push(formula);
            report.promoted += 1;
        }
        report
    }

    fn quarantine(&mut self, formula: Formula, reason: impl Into<String>, confidence: f64) {
        if !self.quarantine.iter().any(|item| item.formula == formula) {
            self.quarantine.push(QuarantinedKnowledge {
                formula,
                reason: reason.into(),
                confidence,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::episodic::EpisodeSource;

    #[test]
    fn repeated_fact_is_promoted() {
        let mut learner = ContinualLearningEngine::default();
        let fact = Formula::prop("ObservedSafe");
        learner.observe(Episode::new(
            EpisodeSource::User,
            "safe",
            Some(fact.clone()),
            0.8,
        ));
        learner.observe(Episode::new(
            EpisodeSource::Environment,
            "safe",
            Some(fact.clone()),
            0.9,
        ));
        assert_eq!(learner.consolidate().promoted, 1);
        assert!(learner.facts.contains(&fact));
    }
}
