//! Multilingual, rule-based NLP front-end for OmiAI.
//!
//! The parser does not try to imitate a neural chatbot. Instead it turns
//! user text into a compact semantic form that can be handed to the logic
//! and memory layers. This keeps the system explainable and preserves the
//! symbolic core.

use std::collections::HashMap;

use crate::core::logic_engine::{Formula, Term};

use super::action::Action;
use super::tokenizer::{Token, tokenize};

/// Supported dialogue languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectedLanguage {
    English,
    Vietnamese,
}

/// High-level parsed intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseIntent {
    Assert,
    Ask,
    Explain,
    Request(Action),
    Greeting,
    Clarify,
    SmallTalk,
}

/// Parsed message with optional logic payload.
#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub language: DetectedLanguage,
    pub intent: ParseIntent,
    pub formula: Option<Formula>,
    pub query: Option<Formula>,
}

/// Rule-based multilingual parser.
#[derive(Debug, Clone)]
pub struct NlpParser {
    lexicon_en: HashMap<String, String>,
    lexicon_vi: HashMap<String, String>,
}

impl Default for NlpParser {
    fn default() -> Self {
        Self::new()
    }
}

impl NlpParser {
    /// Create a parser with a minimal bilingual lexicon.
    pub fn new() -> Self {
        let mut lexicon_en = HashMap::new();
        lexicon_en.insert("human".into(), "Human".into());
        lexicon_en.insert("mortal".into(), "Mortal".into());
        lexicon_en.insert("capital".into(), "CapitalOf".into());
        lexicon_en.insert("hello".into(), "greeting".into());

        let mut lexicon_vi = HashMap::new();
        lexicon_vi.insert("người".into(), "Human".into());
        lexicon_vi.insert("phàm".into(), "Mortal".into());
        lexicon_vi.insert("thủ đô".into(), "CapitalOf".into());
        lexicon_vi.insert("xin chào".into(), "greeting".into());

        Self {
            lexicon_en,
            lexicon_vi,
        }
    }

    /// Infer language using simple heuristics.
    pub fn detect_language(&self, input: &str) -> Option<DetectedLanguage> {
        let lower = input.to_lowercase();
        if lower.contains('á')
            || lower.contains('à')
            || lower.contains('đ')
            || lower.contains("thủ đô")
            || lower.contains("xin chào")
        {
            Some(DetectedLanguage::Vietnamese)
        } else if lower.chars().any(|c| c.is_ascii_alphabetic()) {
            Some(DetectedLanguage::English)
        } else {
            None
        }
    }

    /// Parse a message into an intent and optional logical form.
    pub fn parse_message(
        &self,
        input: &str,
        language: DetectedLanguage,
    ) -> Result<ParsedMessage, String> {
        let tokens = tokenize(input).map_err(|e| format!("tokenize failed: {e}"))?;
        let words = self.extract_words(&tokens);
        let has_question_mark = tokens.iter().any(|token| matches!(token, Token::Question));
        if words.is_empty() {
            return Err("empty input".into());
        }

        if self.is_greeting(&words, language) {
            return Ok(ParsedMessage {
                language,
                intent: ParseIntent::Greeting,
                formula: None,
                query: None,
            });
        }

        if has_question_mark || self.is_question(&words) {
            let query = self.build_query(&words, language)?;
            return Ok(ParsedMessage {
                language,
                intent: ParseIntent::Ask,
                formula: None,
                query: Some(query),
            });
        }

        if self.is_request(&words) {
            let action = Action::new(
                self.normalized_concept(&words[0], language),
                words.iter().skip(1).cloned().collect(),
            );
            return Ok(ParsedMessage {
                language,
                intent: ParseIntent::Request(action),
                formula: None,
                query: None,
            });
        }

        if let Some(formula) = self.build_assertion(&words, language) {
            return Ok(ParsedMessage {
                language,
                intent: ParseIntent::Assert,
                formula: Some(formula),
                query: None,
            });
        }

        Ok(ParsedMessage {
            language,
            intent: ParseIntent::Clarify,
            formula: None,
            query: None,
        })
    }

    fn extract_words(&self, tokens: &[Token]) -> Vec<String> {
        tokens
            .iter()
            .filter_map(|t| match t {
                Token::Ident(s) | Token::StringLit(s) => Some(s.to_lowercase()),
                _ => None,
            })
            .collect()
    }

    fn is_greeting(&self, words: &[String], _language: DetectedLanguage) -> bool {
        words
            .iter()
            .any(|w| matches!(w.as_str(), "hello" | "hi" | "xin" | "chào"))
            || words.join(" ").contains("xin chào")
    }

    fn is_question(&self, words: &[String]) -> bool {
        words
            .iter()
            .any(|w| matches!(w.as_str(), "what" | "why" | "how" | "ai" | "gì" | "tại"))
            || words.last().map(|s| s == "?").unwrap_or(false)
    }

    fn is_request(&self, words: &[String]) -> bool {
        words
            .first()
            .map(|w| matches!(w.as_str(), "please" | "hãy" | "làm" | "thực"))
            .unwrap_or(false)
    }

    fn normalized_concept(&self, word: &str, language: DetectedLanguage) -> String {
        match language {
            DetectedLanguage::English => self
                .lexicon_en
                .get(word)
                .cloned()
                .unwrap_or_else(|| capitalize(word)),
            DetectedLanguage::Vietnamese => self
                .lexicon_vi
                .get(word)
                .cloned()
                .unwrap_or_else(|| capitalize(word)),
        }
    }

    fn build_query(&self, words: &[String], language: DetectedLanguage) -> Result<Formula, String> {
        if words.len() >= 3 && matches!(words[0].as_str(), "what" | "ai") {
            let concept = self.normalized_concept(&words[1], language);
            return Ok(Formula::atom(concept, vec![Term::Var("x".into())]));
        }
        if words.len() >= 3 && matches!(words[1].as_str(), "is" | "là") {
            let predicate = self.normalized_concept(&words[2], language);
            return Ok(Formula::atom(
                predicate,
                vec![Term::Const(words[0].clone())],
            ));
        }
        if words.len() >= 4 && matches!(words[0].as_str(), "is" | "does" | "có") {
            let predicate = self.normalized_concept(
                words.last().map(String::as_str).unwrap_or("unknown"),
                language,
            );
            return Ok(Formula::atom(
                predicate,
                vec![Term::Const(words[1].clone())],
            ));
        }
        Err("cannot build query".into())
    }

    fn build_assertion(&self, words: &[String], language: DetectedLanguage) -> Option<Formula> {
        if words.len() == 3 && matches!(words[1].as_str(), "is" | "là") {
            let pred = self.normalized_concept(&words[2], language);
            return Some(Formula::atom(pred, vec![Term::Const(words[0].clone())]));
        }
        None
    }

    /// Realize a proven assertion into natural language.
    pub fn realize_assertion(&self, formula: &Formula, language: DetectedLanguage) -> String {
        match (formula, language) {
            (Formula::Atom(pred, args), DetectedLanguage::Vietnamese) if args.len() == 1 => {
                format!("{} là {}.", args[0], pred.to_lowercase())
            }
            (Formula::Atom(pred, args), _) if args.len() == 1 => {
                format!("{} is {}.", args[0], pred.to_lowercase())
            }
            _ => self.realize_clarification(language),
        }
    }

    /// Realize a proof result.
    pub fn realize_answer(
        &self,
        formula: &Formula,
        proof: &crate::core::inference::ProofResult,
        language: DetectedLanguage,
    ) -> String {
        match proof {
            crate::core::inference::ProofResult::Proved { .. } => {
                self.realize_assertion(formula, language)
            }
            _ => self.realize_no_answer(language),
        }
    }

    /// Clarification prompt.
    pub fn realize_clarification(&self, language: DetectedLanguage) -> String {
        match language {
            DetectedLanguage::Vietnamese => {
                "Bạn có thể nói rõ hơn để tôi suy luận chính xác không?".into()
            }
            DetectedLanguage::English => "Could you clarify so I can reason precisely?".into(),
        }
    }

    /// No-answer prompt.
    pub fn realize_no_answer(&self, language: DetectedLanguage) -> String {
        match language {
            DetectedLanguage::Vietnamese => "Tôi chưa đủ dữ kiện để kết luận.".into(),
            DetectedLanguage::English => "I do not yet have enough evidence to conclude.".into(),
        }
    }

    /// Realize a greeting.
    pub fn realize_greeting(&self, language: DetectedLanguage) -> String {
        match language {
            DetectedLanguage::Vietnamese => {
                "Xin chào. Tôi có thể suy luận, giải thích hoặc hỗ trợ hành động.".into()
            }
            DetectedLanguage::English => {
                "Hello. I can reason, explain, or help perform actions.".into()
            }
        }
    }

    /// Realize a small-talk response.
    pub fn realize_small_talk(&self, language: DetectedLanguage) -> String {
        match language {
            DetectedLanguage::Vietnamese => "Tôi sẵn sàng hỗ trợ bạn.".into(),
            DetectedLanguage::English => "I am ready to help you.".into(),
        }
    }

    /// Realize an explanation prompt.
    pub fn realize_explanation_prompt(&self, language: DetectedLanguage) -> String {
        match language {
            DetectedLanguage::Vietnamese => {
                "Tôi có thể giải thích bằng chứng hoặc khái niệm liên quan.".into()
            }
            DetectedLanguage::English => "I can explain the proof or the related concept.".into(),
        }
    }

    /// Realize a request to act.
    pub fn realize_action(&self, action: &Action, language: DetectedLanguage) -> String {
        super::chat::describe_action(action, language)
    }

    /// Realize a parser error message.
    pub fn realize_error(&self, _err: &str, language: DetectedLanguage) -> String {
        self.realize_clarification(language)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_vietnamese() {
        let p = NlpParser::new();
        assert_eq!(
            p.detect_language("xin chào"),
            Some(DetectedLanguage::Vietnamese)
        );
    }

    #[test]
    fn parses_english_greeting() {
        let p = NlpParser::new();
        let msg = p.parse_message("hello", DetectedLanguage::English).unwrap();
        assert_eq!(msg.intent, ParseIntent::Greeting);
    }
}
