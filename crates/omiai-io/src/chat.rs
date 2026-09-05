//! Multilingual chat front-end for OmiAI.
//!
//! This layer turns raw user text into a structured dialogue turn, routes it
//! through all 8 reasoning pillars (core, knowledge, probabilistic, causal,
//! neuro, world, evolution, meta), and realizes an answer back into English or
//! Vietnamese without replacing the reasoning engine.

use omiai_core::inference::ProofResult;
use omiai_core::logic_engine::{Formula, Term};
use omiai_core::prover::TheoremProver;
use crate::conversation::ConversationMemory;
use crate::dialogue_router::{DialogueRouter, ReasoningResult};

use super::action::Action;
use super::nlp_parser::{DetectedLanguage, NlpParser, ParseIntent};

/// A user-visible chat request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRequest {
    pub text: String,
    pub preferred_language: Option<DetectedLanguage>,
}

/// A user-visible chat response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResponse {
    pub language: DetectedLanguage,
    pub text: String,
    pub intent: ParseIntent,
    pub proven: bool,
    pub confidence: u8,
}

/// High-level chat engine.
#[derive(Debug)]
pub struct ChatEngine {
    parser: NlpParser,
    prover: TheoremProver,
    router: DialogueRouter,
}

impl Default for ChatEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatEngine {
    /// Create a chat engine with the default multilingual lexicon.
    pub fn new() -> Self {
        Self {
            parser: NlpParser::default(),
            prover: TheoremProver::new(),
            router: DialogueRouter::new(),
        }
    }

    /// Replace the dialogue router (used when loading from checkpoint).
    pub fn set_router(&mut self, router: DialogueRouter) {
        self.router = router;
    }

    /// Handle a request, updating memory and returning a natural-language reply.
    pub fn handle(&mut self, request: &ChatRequest, memory: &mut ConversationMemory) -> ChatResponse {
        let detected = request
            .preferred_language
            .or_else(|| self.parser.detect_language(&request.text));
        let lang = detected.unwrap_or(DetectedLanguage::English);
        let parsed = self.parser.parse_message(&request.text, lang);

        memory.push_user(&request.text, lang);

        let (intent, reply, reasoning_result) = match parsed {
            Ok(message) => self.respond_to_message(message, memory),
            Err(err) => {
                let text = self.parser.realize_error(&err, lang);
                (ParseIntent::Clarify, text, ReasoningResult::NoAnswer)
            }
        };

        memory.push_assistant(&reply, lang);
        let (proven, confidence) = self.assess_confidence(&reasoning_result);
        ChatResponse {
            language: lang,
            text: reply,
            intent,
            proven,
            confidence,
        }
    }

    fn respond_to_message(
        &mut self,
        message: super::nlp_parser::ParsedMessage,
        memory: &mut ConversationMemory,
    ) -> (ParseIntent, String, ReasoningResult) {
        let facts = memory.facts();
        let query_type = message.query_type;
        let routing_result = self.router.route(
            &message.intent,
            message.formula.as_ref(),
            message.query.as_ref(),
            &facts,
            query_type,
        );

        // Also store assertion in memory if it's an Assert
        if matches!(message.intent, ParseIntent::Assert) {
            if let Some(formula) = &message.formula {
                if let Formula::Atom(_, args) = formula
                    && let Some(Term::Const(entity)) = args.first() {
                        memory.focus_entity(entity.clone());
                    }
                memory.push_fact(formula.clone());
            }
        }

        let reply = self.parser.realize_reasoning_result(&routing_result, message.language);
        (message.intent, reply, routing_result)
    }

    fn assess_confidence(&self, result: &ReasoningResult) -> (bool, u8) {
        match result {
            ReasoningResult::LogicalProof { proof, .. } => {
                let proven = matches!(proof, ProofResult::Proved { .. });
                (proven, if proven { 100 } else { 35 })
            }
            ReasoningResult::Probabilistic { probability, .. } => {
                // Confidence based on how far from 0.5
                let conf = ((probability - 0.5).abs() * 200.0) as u8;
                (false, conf.clamp(10, 90))
            }
            ReasoningResult::Causal { explanation, .. } => {
                (explanation.is_causal, if explanation.is_causal { 85 } else { 40 })
            }
            ReasoningResult::KnowledgeGraph { result, .. } => {
                let has_result = !matches!(result, crate::dialogue_router::KnowledgeResult::NotFound);
                (has_result, if has_result { 80 } else { 30 })
            }
            ReasoningResult::NoAnswer => (false, 10),
        }
    }
}

/// Convert a discrete action to a chat-friendly line.
pub fn describe_action(action: &Action, language: DetectedLanguage) -> String {
    match language {
        DetectedLanguage::Vietnamese => {
            format!(
                "Tôi sẽ thực hiện hành động `{}` với {} tham số.",
                action.name,
                action.args.len()
            )
        }
        _ => format!(
            "I will execute action `{}` with {} argument(s).",
            action.name,
            action.args.len()
        ),
    }
}

/// Create a simple self-contained greeting response.
pub fn greet(language: DetectedLanguage) -> String {
    match language {
        DetectedLanguage::Vietnamese => {
            "Xin chào. Tôi là OmiAI. Bạn muốn tôi suy luận, giải thích hay thực hiện hành động gì?"
                .into()
        }
        _ => {
            "Hello. I am OmiAI. Would you like me to reason, explain, or perform an action?".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::ConversationMemory;

    #[test]
    fn chat_engine_greets_in_english() {
        let mut engine = ChatEngine::new();
        let mut memory = ConversationMemory::default();
        let response = engine.handle(
            &ChatRequest {
                text: "hello".into(),
                preferred_language: None,
            },
            &mut memory,
        );
        assert!(!response.text.is_empty());
    }
}
