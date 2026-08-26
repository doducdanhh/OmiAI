//! Episodic memory for continual symbolic learning.
//!
//! Episodes preserve observations, provenance, confidence and outcome so that
//! repeated evidence can later be consolidated into semantic knowledge.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::logic_engine::Formula;
use crate::io::nlp_parser::DetectedLanguage;

/// Origin of an observed episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpisodeSource {
    User,
    ImportedFile(String),
    DerivedProof,
    Environment,
}

/// One inspectable learning event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: EpisodeSource,
    pub language: Option<String>,
    pub raw_observation: String,
    pub formula: Option<Formula>,
    pub confidence: f64,
    pub confirmed: bool,
}

impl Episode {
    /// Construct an episode and clamp confidence to a valid probability.
    pub fn new(
        source: EpisodeSource,
        raw_observation: impl Into<String>,
        formula: Option<Formula>,
        confidence: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            source,
            language: None,
            raw_observation: raw_observation.into(),
            formula,
            confidence: confidence.clamp(0.0, 1.0),
            confirmed: false,
        }
    }

    /// Attach a detected dialogue language.
    pub fn with_language(mut self, language: DetectedLanguage) -> Self {
        self.language = Some(
            match language {
                DetectedLanguage::English => "en",
                DetectedLanguage::Vietnamese => "vi",
            }
            .into(),
        );
        self
    }
}

/// Append-oriented episodic store.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EpisodicMemory {
    episodes: Vec<Episode>,
}

impl EpisodicMemory {
    /// Create empty episodic memory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event and return its stable identifier.
    pub fn remember(&mut self, episode: Episode) -> Uuid {
        let id = episode.id;
        self.episodes.push(episode);
        id
    }

    /// Access all events in chronological insertion order.
    pub fn episodes(&self) -> &[Episode] {
        &self.episodes
    }

    /// Mark an event as externally confirmed.
    pub fn confirm(&mut self, id: Uuid) -> bool {
        if let Some(episode) = self.episodes.iter_mut().find(|episode| episode.id == id) {
            episode.confirmed = true;
            episode.confidence = episode.confidence.max(0.95);
            true
        } else {
            false
        }
    }

    /// Return candidate formulas supported by at least `minimum_observations` episodes.
    pub fn consolidation_candidates(
        &self,
        minimum_observations: usize,
        minimum_confidence: f64,
    ) -> Vec<(Formula, usize, f64)> {
        let mut groups: Vec<(Formula, usize, f64)> = Vec::new();
        for episode in self
            .episodes
            .iter()
            .filter(|episode| episode.confidence >= minimum_confidence)
        {
            let Some(formula) = &episode.formula else {
                continue;
            };
            if let Some((_, count, confidence_sum)) =
                groups.iter_mut().find(|(known, _, _)| known == formula)
            {
                *count += 1;
                *confidence_sum += episode.confidence;
            } else {
                groups.push((formula.clone(), 1, episode.confidence));
            }
        }
        groups
            .into_iter()
            .filter(|(_, count, _)| *count >= minimum_observations)
            .map(|(formula, count, sum)| (formula, count, sum / count as f64))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_observations_become_candidates() {
        let fact = Formula::prop("Safe");
        let mut memory = EpisodicMemory::new();
        memory.remember(Episode::new(
            EpisodeSource::User,
            "safe",
            Some(fact.clone()),
            0.8,
        ));
        memory.remember(Episode::new(
            EpisodeSource::Environment,
            "safe",
            Some(fact),
            0.9,
        ));
        assert_eq!(memory.consolidation_candidates(2, 0.5).len(), 1);
    }
}
