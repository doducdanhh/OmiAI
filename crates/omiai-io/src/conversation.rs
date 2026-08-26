//! Dialogue memory: conversation turns, remembered facts, salient entities.
//!
//! (Moved from `meta::introspection` to break the io↔meta dependency cycle —
//! see ADR-0005. The only meaningful consumer is the chat front-end.)

use omiai_core::logic_engine::Formula;

use super::nlp_parser::DetectedLanguage;

/// A fact retained with conversational provenance.
#[derive(Debug, Clone)]
pub struct RememberedFact {
    pub formula: Formula,
    pub source_turn: usize,
    pub confidence: f64,
}

/// Memory of a dialogue session.
#[derive(Debug, Default, Clone)]
pub struct ConversationMemory {
    user_turns: Vec<(String, DetectedLanguage)>,
    assistant_turns: Vec<(String, DetectedLanguage)>,
    facts: Vec<RememberedFact>,
    active_entities: Vec<String>,
}

impl ConversationMemory {
    /// Push a user turn.
    pub fn push_user(&mut self, text: impl Into<String>, language: DetectedLanguage) {
        self.user_turns.push((text.into(), language));
    }

    /// Push an assistant turn.
    pub fn push_assistant(&mut self, text: impl Into<String>, language: DetectedLanguage) {
        self.assistant_turns.push((text.into(), language));
    }

    /// Store a fact derived from the conversation.
    pub fn push_fact(&mut self, fact: Formula) {
        self.push_fact_with_confidence(fact, 1.0);
    }

    /// Store a fact together with an evidence confidence in \([0,1]\).
    pub fn push_fact_with_confidence(&mut self, fact: Formula, confidence: f64) {
        let source_turn = self.user_turns.len().saturating_sub(1);
        let confidence = confidence.clamp(0.0, 1.0);
        if !self.facts.iter().any(|known| known.formula == fact) {
            self.facts.push(RememberedFact {
                formula: fact,
                source_turn,
                confidence,
            });
        }
    }

    /// Return formulas suitable for symbolic proof search.
    pub fn facts(&self) -> Vec<Formula> {
        self.facts
            .iter()
            .map(|known| known.formula.clone())
            .collect()
    }

    /// Access facts together with their provenance.
    pub fn remembered_facts(&self) -> &[RememberedFact] {
        &self.facts
    }

    /// Mark an entity as salient for later pronoun/reference resolution.
    pub fn focus_entity(&mut self, entity: impl Into<String>) {
        let entity = entity.into();
        self.active_entities.retain(|known| known != &entity);
        self.active_entities.push(entity);
    }

    /// Return the most recently focused entity.
    pub fn focused_entity(&self) -> Option<&str> {
        self.active_entities.last().map(String::as_str)
    }

    /// Infer the last seen language, if any.
    pub fn last_language(&self) -> Option<DetectedLanguage> {
        self.assistant_turns
            .last()
            .map(|(_, lang)| *lang)
            .or_else(|| self.user_turns.last().map(|(_, lang)| *lang))
    }
}
