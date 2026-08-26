//! Multilingual chat front-end for OmiAI.
//!
//! This layer turns raw user text into a structured dialogue turn, routes it
//! through the symbolic core, and realizes an answer back into English or
//! Vietnamese without replacing the reasoning engine.

use omiai_core::inference::ProofResult;
use omiai_core::logic_engine::{Formula, Term};
use omiai_core::prover::TheoremProver;
use crate::conversation::ConversationMemory;

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
#[derive(Debug, Clone)]
pub struct ChatEngine {
    parser: NlpParser,
    prover: TheoremProver,
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
        }
    }

    /// Handle a request, updating memory and returning a natural-language reply.
    pub fn handle(&self, request: &ChatRequest, memory: &mut ConversationMemory) -> ChatResponse {
        let detected = request
            .preferred_language
            .or_else(|| self.parser.detect_language(&request.text));
        let lang = detected.unwrap_or(DetectedLanguage::English);
        let parsed = self.parser.parse_message(&request.text, lang);

        memory.push_user(&request.text, lang);

        let (intent, reply, proven) = match parsed {
            Ok(message) => self.respond_to_message(message, memory),
            Err(err) => {
                let text = self.parser.realize_error(&err, lang);
                (ParseIntent::Clarify, text, false)
            }
        };

        memory.push_assistant(&reply, lang);
        ChatResponse {
            language: lang,
            text: reply,
            intent,
            proven,
            confidence: if proven { 100 } else { 35 },
        }
    }

    fn respond_to_message(
        &self,
        message: super::nlp_parser::ParsedMessage,
        memory: &mut ConversationMemory,
    ) -> (ParseIntent, String, bool) {
        match message.intent {
            ParseIntent::Assert => {
                if let Some(formula) = message.formula {
                    if let Formula::Atom(_, args) = &formula
                        && let Some(Term::Const(entity)) = args.first() {
                            memory.focus_entity(entity.clone());
                        }
                    memory.push_fact(formula.clone());
                    (
                        ParseIntent::Assert,
                        self.parser.realize_assertion(&formula, message.language),
                        true,
                    )
                } else {
                    (
                        ParseIntent::Clarify,
                        self.parser.realize_clarification(message.language),
                        false,
                    )
                }
            }
            ParseIntent::Ask => {
                if let Some(query) = message.query {
                    let proof = self.answer_query(&query, memory);
                    match proof {
                        Some((formula, proof)) => (
                            ParseIntent::Ask,
                            self.parser
                                .realize_answer(&formula, &proof, message.language),
                            true,
                        ),
                        None => (
                            ParseIntent::Clarify,
                            self.parser.realize_no_answer(message.language),
                            false,
                        ),
                    }
                } else {
                    (
                        ParseIntent::Clarify,
                        self.parser.realize_clarification(message.language),
                        false,
                    )
                }
            }
            ParseIntent::Request(action) => {
                let rendered = self.parser.realize_action(&action, message.language);
                (ParseIntent::Request(action), rendered, false)
            }
            ParseIntent::Greeting => (
                ParseIntent::Greeting,
                self.parser.realize_greeting(message.language),
                false,
            ),
            ParseIntent::Explain => (
                ParseIntent::Explain,
                self.parser.realize_explanation_prompt(message.language),
                false,
            ),
            ParseIntent::Clarify => (
                ParseIntent::Clarify,
                self.parser.realize_clarification(message.language),
                false,
            ),
            ParseIntent::SmallTalk => (
                ParseIntent::SmallTalk,
                self.parser.realize_small_talk(message.language),
                false,
            ),
        }
    }

    fn answer_query(
        &self,
        query: &Formula,
        memory: &ConversationMemory,
    ) -> Option<(Formula, ProofResult)> {
        let premises = memory.facts();
        let proof = self.prover.prove(&premises, query);
        if matches!(proof, ProofResult::Proved { .. }) {
            return Some((query.clone(), proof));
        }

        let negated = Formula::Not(Box::new(query.clone()));
        let negative_proof = self.prover.prove(&premises, &negated);
        if matches!(negative_proof, ProofResult::Proved { .. }) {
            Some((negated, negative_proof))
        } else {
            None
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
        let engine = ChatEngine::new();
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
