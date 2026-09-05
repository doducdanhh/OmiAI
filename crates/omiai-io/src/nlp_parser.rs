//! Multilingual, rule-based NLP front-end for OmiAI.
//!
//! The parser does not try to imitate a neural chatbot. Instead it turns
//! user text into a compact semantic form that can be handed to the logic
//! and memory layers. This keeps the system explainable and preserves the
//! symbolic core.

use std::collections::HashMap;

use omiai_core::inference::ProofResult;
use omiai_core::logic_engine::{Formula, Term};
use crate::dialogue_router::{CausalExplanation, CausalQuery, KnowledgeQuery, KnowledgeResult, ProbMethod, ReasoningResult};

use super::action::Action;
use super::tokenizer::{Token, tokenize};

/// Keywords that trigger probabilistic reasoning pillar.
const PROB_KEYWORDS_EN: &[&str] = &[
    "probably", "likely", "chance", "probability", "probabilistic", "uncertain", "maybe", "perhaps"
];
const PROB_KEYWORDS_VI: &[&str] = &[
    "có lẽ", "khả năng", "xác suất", "chắc chắn", "dù có", "có thể"
];

/// Keywords that trigger causal reasoning pillar.
const CAUSAL_KEYWORDS_EN: &[&str] = &[
    "why", "because", "cause", "causal", "what if", "counterfactual", "if "
];
const CAUSAL_KEYWORDS_VI: &[&str] = &[
    "tại sao", "bởi vì", "nguyên nhân", "nhân quả", "nếu", "giả sử", "phản sự kiện"
];

/// Keywords that trigger world query pillar.
const WORLD_KEYWORDS_EN: &[&str] = &[
    "agent", "population", "world", "how many", "vocabulary", "convention", "symbol", "emergent"
];
const WORLD_KEYWORDS_VI: &[&str] = &[
    "agent", "thế giới", "bao nhiêu", "từ vựng", "quy ước", "ký hiệu", "nổi sinh"
];

/// Keywords that trigger knowledge graph queries.
const KG_KEYWORDS_EN: &[&str] = &[
    "related", "connected", "path", "is a", "is an", "type of", "kind of", "instance"
];
const KG_KEYWORDS_VI: &[&str] = &[
    "liên quan", "liên kết", "đường", "là", "loại", "thể loại", "ví dụ"
];

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
    pub query_type: QueryType,
}

/// Rule-based multilingual parser.
#[derive(Debug, Clone)]
pub struct NlpParser {
    lexicon_en: HashMap<String, String>,
    lexicon_vi: HashMap<String, String>,
    /// Extended concept vocabulary loaded from knowledge graph
    extended_concepts: HashMap<String, String>,
}

/// Type of query detected for routing to appropriate pillar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Standard logical query (default)
    Logical,
    /// Probabilistic query (e.g., "what is the probability of rain?")
    Probabilistic,
    /// Causal query (e.g., "why does X cause Y?")
    Causal,
    /// World query (e.g., "how many agents?")
    World,
    /// Knowledge graph query (e.g., "is sparrow related to bird?")
    KnowledgeGraph,
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
        // Extended English concepts
        lexicon_en.insert("person".into(), "Human".into());
        lexicon_en.insert("people".into(), "Human".into());
        lexicon_en.insert("man".into(), "Human".into());
        lexicon_en.insert("woman".into(), "Human".into());
        lexicon_en.insert("animal".into(), "Animal".into());
        lexicon_en.insert("bird".into(), "Bird".into());
        lexicon_en.insert("sparrow".into(), "Sparrow".into());
        lexicon_en.insert("cat".into(), "Cat".into());
        lexicon_en.insert("dog".into(), "Dog".into());
        lexicon_en.insert("fly".into(), "Fly".into());
        lexicon_en.insert("fly".into(), "CanFly".into());
        lexicon_en.insert("wings".into(), "HasWings".into());
        lexicon_en.insert("rain".into(), "Rain".into());
        lexicon_en.insert("wet".into(), "Wet".into());
        lexicon_en.insert("grass".into(), "Grass".into());
        lexicon_en.insert("sun".into(), "Sun".into());
        lexicon_en.insert("hot".into(), "Hot".into());
        lexicon_en.insert("cold".into(), "Cold".into());
        lexicon_en.insert("know".into(), "Know".into());
        lexicon_en.insert("believe".into(), "Believe".into());
        lexicon_en.insert("think".into(), "Think".into());
        lexicon_en.insert("agent".into(), "Agent".into());
        lexicon_en.insert("population".into(), "Population".into());
        lexicon_en.insert("world".into(), "World".into());
        lexicon_en.insert("symbol".into(), "Symbol".into());
        lexicon_en.insert("convention".into(), "Convention".into());
        lexicon_en.insert("vocabulary".into(), "Vocabulary".into());
        lexicon_en.insert("emergent".into(), "Emergent".into());
        lexicon_en.insert("related".into(), "Related".into());
        lexicon_en.insert("connected".into(), "Connected".into());
        lexicon_en.insert("path".into(), "Path".into());
        lexicon_en.insert("type".into(), "TypeOf".into());
        lexicon_en.insert("kind".into(), "KindOf".into());
        lexicon_en.insert("example".into(), "InstanceOf".into());
        lexicon_en.insert("instance".into(), "InstanceOf".into());

        let mut lexicon_vi = HashMap::new();
        lexicon_vi.insert("người".into(), "Human".into());
        lexicon_vi.insert("phàm".into(), "Mortal".into());
        lexicon_vi.insert("thủ đô".into(), "CapitalOf".into());
        lexicon_vi.insert("xin chào".into(), "greeting".into());
        // Extended Vietnamese concepts
        lexicon_vi.insert("người ta".into(), "Human".into());
        lexicon_vi.insert("con người".into(), "Human".into());
        lexicon_vi.insert("người đàn ông".into(), "Human".into());
        lexicon_vi.insert("người phụ nữ".into(), "Human".into());
        lexicon_vi.insert("động vật".into(), "Animal".into());
        lexicon_vi.insert("chim".into(), "Bird".into());
        lexicon_vi.insert("chim sẻ".into(), "Sparrow".into());
        lexicon_vi.insert("mèo".into(), "Cat".into());
        lexicon_vi.insert("chó".into(), "Dog".into());
        lexicon_vi.insert("bay".into(), "Fly".into());
        lexicon_vi.insert("có cánh".into(), "HasWings".into());
        lexicon_vi.insert("mưa".into(), "Rain".into());
        lexicon_vi.insert("ướt".into(), "Wet".into());
        lexicon_vi.insert("cỏ".into(), "Grass".into());
        lexicon_vi.insert("nắng".into(), "Sun".into());
        lexicon_vi.insert("nóng".into(), "Hot".into());
        lexicon_vi.insert("lạnh".into(), "Cold".into());
        lexicon_vi.insert("biết".into(), "Know".into());
        lexicon_vi.insert("tin".into(), "Believe".into());
        lexicon_vi.insert("nghĩ".into(), "Think".into());
        lexicon_vi.insert("agent".into(), "Agent".into());
        lexicon_vi.insert("dân số".into(), "Population".into());
        lexicon_vi.insert("thế giới".into(), "World".into());
        lexicon_vi.insert("ký hiệu".into(), "Symbol".into());
        lexicon_vi.insert("quy ước".into(), "Convention".into());
        lexicon_vi.insert("từ vựng".into(), "Vocabulary".into());
        lexicon_vi.insert("nổi sinh".into(), "Emergent".into());
        lexicon_vi.insert("liên quan".into(), "Related".into());
        lexicon_vi.insert("liên kết".into(), "Connected".into());
        lexicon_vi.insert("đường".into(), "Path".into());
        lexicon_vi.insert("loại".into(), "TypeOf".into());
        lexicon_vi.insert("thể loại".into(), "KindOf".into());
        lexicon_vi.insert("ví dụ".into(), "InstanceOf".into());

        Self {
            lexicon_en,
            lexicon_vi,
            extended_concepts: HashMap::new(),
        }
    }

    /// Load extended vocabulary from knowledge graph (called after graph is populated)
    pub fn load_extended_vocabulary(&mut self, concepts: &[(String, String)]) {
        for (id, label) in concepts {
            // Map lowercase label to concept id
            self.extended_concepts.insert(label.to_lowercase(), id.clone());
        }
    }

    /// Check if a word is a known concept (in base lexicon or extended)
    fn is_known_concept(&self, word: &str, language: DetectedLanguage) -> bool {
        let lower = word.to_lowercase();
        match language {
            DetectedLanguage::English => self.lexicon_en.contains_key(&lower) || self.extended_concepts.contains_key(&lower),
            DetectedLanguage::Vietnamese => self.lexicon_vi.contains_key(&lower) || self.extended_concepts.contains_key(&lower),
        }
    }

    /// Get normalized concept from base or extended lexicon
    fn normalized_concept(&self, word: &str, language: DetectedLanguage) -> String {
        let lower = word.to_lowercase();
        match language {
            DetectedLanguage::English => self
                .lexicon_en
                .get(&lower)
                .cloned()
                .or_else(|| self.extended_concepts.get(&lower).cloned())
                .unwrap_or_else(|| capitalize(&lower)),
            DetectedLanguage::Vietnamese => self
                .lexicon_vi
                .get(&lower)
                .cloned()
                .or_else(|| self.extended_concepts.get(&lower).cloned())
                .unwrap_or_else(|| capitalize(&lower)),
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
                query_type: QueryType::Logical,
            });
        }

        if has_question_mark || self.is_question(&words) {
            let query = self.build_query(&words, language)?;
            let query_type = self.detect_query_type(&words, language);
            return Ok(ParsedMessage {
                language,
                intent: ParseIntent::Ask,
                formula: None,
                query: Some(query),
                query_type,
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
                query_type: QueryType::Logical,
            });
        }

        if let Some(formula) = self.build_assertion(&words, language) {
            return Ok(ParsedMessage {
                language,
                intent: ParseIntent::Assert,
                formula: Some(formula),
                query: None,
                query_type: QueryType::Logical,
            });
        }

        Ok(ParsedMessage {
            language,
            intent: ParseIntent::Clarify,
            formula: None,
            query: None,
            query_type: QueryType::Logical,
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

    /// Detect the type of query based on keywords to route to appropriate pillar.
    fn detect_query_type(&self, words: &[String], language: DetectedLanguage) -> QueryType {
        let text = words.join(" ").to_lowercase();

        let (prob_kw, causal_kw, world_kw, kg_kw) = match language {
            DetectedLanguage::English => (PROB_KEYWORDS_EN, CAUSAL_KEYWORDS_EN, WORLD_KEYWORDS_EN, KG_KEYWORDS_EN),
            DetectedLanguage::Vietnamese => (PROB_KEYWORDS_VI, CAUSAL_KEYWORDS_VI, WORLD_KEYWORDS_VI, KG_KEYWORDS_VI),
        };

        // Check probabilistic keywords first
        if prob_kw.iter().any(|kw| text.contains(kw)) {
            return QueryType::Probabilistic;
        }
        // Check causal keywords
        if causal_kw.iter().any(|kw| text.contains(kw)) {
            return QueryType::Causal;
        }
        // Check world keywords
        if world_kw.iter().any(|kw| text.contains(kw)) {
            return QueryType::World;
        }
        // Check knowledge graph keywords
        if kg_kw.iter().any(|kw| text.contains(kw)) {
            return QueryType::KnowledgeGraph;
        }

        QueryType::Logical
    }

    fn build_query(&self, words: &[String], language: DetectedLanguage) -> Result<Formula, String> {
        // "what is X" / "X là gì" / "ai là X"
        if words.len() >= 3 && matches!(words[0].as_str(), "what" | "ai") {
            let concept = self.normalized_concept(&words[1], language);
            return Ok(Formula::atom(concept, vec![Term::Var("x".into())]));
        }
        // "what does X do" / "X làm gì"
        if words.len() >= 4 && matches!(words[0].as_str(), "what") && matches!(words[1].as_str(), "does" | "làm") {
            let concept = self.normalized_concept(&words[2], language);
            return Ok(Formula::atom(concept, vec![Term::Var("x".into())]));
        }
        // "X is Y" / "X là Y" / "X is a Y" / "X là một Y"
        if words.len() >= 3 && matches!(words[1].as_str(), "is" | "là") {
            let pred_idx = if words[2] == "a" || words[2] == "một" { 3 } else { 2 };
            if pred_idx < words.len() {
                let predicate = self.normalized_concept(&words[pred_idx], language);
                return Ok(Formula::atom(
                    predicate,
                    vec![Term::Const(words[0].clone())],
                ));
            }
        }
        // "does X Y" / "có X Y" / "is X Y" (3 words: is X Y)
        if words.len() >= 3 && matches!(words[0].as_str(), "is" | "does" | "có") {
            let predicate = self.normalized_concept(
                words.last().map(String::as_str).unwrap_or("unknown"),
                language,
            );
            return Ok(Formula::atom(
                predicate,
                vec![Term::Const(words[1].clone())],
            ));
        }
        // "X has Y" / "X có Y"
        if words.len() == 3 && matches!(words[1].as_str(), "has" | "có") {
            let predicate = self.normalized_concept(&words[2], language);
            return Ok(Formula::atom(
                predicate,
                vec![Term::Const(words[0].clone())],
            ));
        }
        // "can X Y" / "có thể X Y"
        if words.len() >= 3 && matches!(words[1].as_str(), "can" | "có thể") {
            let predicate = self.normalized_concept(&words[2], language);
            return Ok(Formula::atom(
                predicate,
                vec![Term::Const(words[0].clone())],
            ));
        }
        // "how many X" / "bao nhiêu X"
        if words.len() >= 3 && matches!(words[0].as_str(), "how" | "bao") && matches!(words[1].as_str(), "many" | "nhiêu") {
            let predicate = self.normalized_concept(&words[2], language);
            return Ok(Formula::atom(predicate, vec![Term::Var("x".into())]));
        }
        // "what is the X of Y" / "X của Y là gì"
        if words.len() >= 5 && matches!(words[0].as_str(), "what" | "ai") && words[2] == "of" {
            let concept = self.normalized_concept(&words[1], language);
            let entity = self.normalized_concept(&words[3], language);
            return Ok(Formula::atom(concept, vec![Term::Const(entity)]));
        }
        // Fallback: use first word as predicate, rest as variable
        if words.len() >= 2 {
            let predicate = self.normalized_concept(&words[0], language);
            return Ok(Formula::atom(predicate, vec![Term::Var("x".into())]));
        }
        // Ultimate fallback: single word as predicate
        if words.len() == 1 {
            let predicate = self.normalized_concept(&words[0], language);
            return Ok(Formula::atom(predicate, vec![Term::Var("x".into())]));
        }
        Err("cannot build query".into())
    }

    fn build_assertion(&self, words: &[String], language: DetectedLanguage) -> Option<Formula> {
        // Universal rule: "every human is mortal" / "mọi người là phàm"
        //   → ∀x (Human(x) → Mortal(x))
        if words.len() == 4
            && matches!(words[0].as_str(), "every" | "mọi" | "all" | "tất cả")
            && matches!(words[2].as_str(), "is" | "là" | "are" | "có")
        {
            let subject = self.normalized_concept(&words[1], language);
            let predicate = self.normalized_concept(&words[3], language);
            let var = || Term::Var("x".into());
            return Some(Formula::ForAll(
                "x".into(),
                Box::new(Formula::Implies(
                    Box::new(Formula::atom(subject, vec![var()])),
                    Box::new(Formula::atom(predicate, vec![var()])),
                )),
            ));
        }
        // "X is a Y" / "X là Y" / "X là một Y"
        if words.len() >= 3 && matches!(words[1].as_str(), "is" | "là" | "is a" | "là một") {
            let pred_idx = if words[2] == "a" || words[2] == "một" { 3 } else { 2 };
            if pred_idx < words.len() {
                let pred = self.normalized_concept(&words[pred_idx], language);
                return Some(Formula::atom(pred, vec![Term::Const(words[0].clone())]));
            }
        }
        // "X has Y" / "X có Y"
        if words.len() == 3 && matches!(words[1].as_str(), "has" | "có") {
            let pred = self.normalized_concept(&words[2], language);
            return Some(Formula::atom(pred, vec![Term::Const(words[0].clone())]));
        }
        // "X can Y" / "X có thể Y"
        if words.len() >= 3 && matches!(words[1].as_str(), "can" | "có thể") {
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
        proof: &omiai_core::inference::ProofResult,
        language: DetectedLanguage,
    ) -> String {
        match proof {
            omiai_core::inference::ProofResult::Proved { .. } => {
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

    /// Realize a unified reasoning result into natural language.
    pub fn realize_reasoning_result(
        &self,
        result: &ReasoningResult,
        language: DetectedLanguage,
    ) -> String {
        match result {
            ReasoningResult::LogicalProof { query, proof, premises_used } => {
                self.realize_logical_proof(query, proof, premises_used, language)
            }
            ReasoningResult::Probabilistic {
                query,
                probability,
                method,
                evidence,
            } => self.realize_probabilistic(query, *probability, *method, evidence, language),
            ReasoningResult::Causal { query, explanation } => {
                self.realize_causal(query, explanation, language)
            }
            ReasoningResult::KnowledgeGraph { query, result } => {
                self.realize_knowledge_graph(query, result, language)
            }
            ReasoningResult::NoAnswer => self.realize_no_answer(language),
        }
    }

    fn realize_logical_proof(
        &self,
        query: &Formula,
        proof: &ProofResult,
        _premises: &[Formula],
        language: DetectedLanguage,
    ) -> String {
        match proof {
            ProofResult::Proved { .. } => self.realize_assertion(query, language),
            omiai_core::inference::ProofResult::Disproved { .. } => self.realize_refutation(query, language),
            _ => self.realize_no_answer(language),
        }
    }

    fn realize_probabilistic(
        &self,
        query: &str,
        probability: f64,
        method: ProbMethod,
        _evidence: &std::collections::HashMap<String, bool>,
        language: DetectedLanguage,
    ) -> String {
        let pct = (probability * 100.0).round() as u32;
        let method_str = match method {
            ProbMethod::Exact => "exact inference",
            ProbMethod::MCMC => "MCMC sampling",
        };
        match language {
            DetectedLanguage::Vietnamese => {
                format!(
                    "Xác suất {} là {:.0}% (theo {}).",
                    query, pct, method_str
                )
            }
            _ => {
                format!(
                    "Probability of {} is {:.0}% (via {}).",
                    query, pct, method_str
                )
            }
        }
    }

    fn realize_causal(
        &self,
        _query: &CausalQuery,
        explanation: &CausalExplanation,
        language: DetectedLanguage,
    ) -> String {
        match language {
            DetectedLanguage::Vietnamese => {
                if explanation.is_causal {
                    format!(
                        "Có quan hệ nhân quả: {}. {}",
                        explanation.details,
                        if explanation.adjustment_set.is_some() {
                            "Điều chỉnh bằng tập back-door."
                        } else {
                            ""
                        }
                    )
                } else {
                    format!("Không tìm thấy quan hệ nhân quả: {}", explanation.details)
                }
            }
            _ => {
                if explanation.is_causal {
                    format!(
                        "Causal relationship found: {}. {}",
                        explanation.details,
                        if explanation.adjustment_set.is_some() {
                            "Adjusted via back-door set."
                        } else {
                            ""
                        }
                    )
                } else {
                    format!("No causal relationship: {}", explanation.details)
                }
            }
        }
    }

    fn realize_knowledge_graph(
        &self,
        query: &KnowledgeQuery,
        result: &KnowledgeResult,
        language: DetectedLanguage,
    ) -> String {
        match (query, result) {
            (KnowledgeQuery::Path { from, to }, KnowledgeResult::Path(path)) => {
                let path_str = path.join(" → ");
                match language {
                    DetectedLanguage::Vietnamese => {
                        format!("Đường đi từ {} đến {}: {}", from, to, path_str)
                    }
                    _ => {
                        format!("Path from {} to {}: {}", from, to, path_str)
                    }
                }
            }
            (KnowledgeQuery::Transitive { relation }, KnowledgeResult::TransitiveClosure(pairs)) => {
                let count = pairs.len();
                match language {
                    DetectedLanguage::Vietnamese => {
                        format!("Đóng bao bắc cầu của '{}': {} cặp.", relation, count)
                    }
                    _ => {
                        format!("Transitive closure of '{}': {} pairs.", relation, count)
                    }
                }
            }
            (KnowledgeQuery::ConsistencyCheck, KnowledgeResult::Consistency(consistent)) => {
                match language {
                    DetectedLanguage::Vietnamese => {
                        format!(
                            "Đồ thị tri thức {} nhất quán.",
                            if *consistent { "là" } else { "KHÔNG" }
                        )
                    }
                    _ => {
                        format!(
                            "Knowledge graph is {} consistent.",
                            if *consistent { "" } else { "NOT" }
                        )
                    }
                }
            }
            (KnowledgeQuery::Subgraph { concepts }, KnowledgeResult::Subgraph(subgraph)) => {
                let count = subgraph.len();
                let concept_list = concepts.join(", ");
                match language {
                    DetectedLanguage::Vietnamese => {
                        format!(
                            "Tìm thấy {} khái niệm liên quan đến: {}.",
                            count, concept_list
                        )
                    }
                    _ => {
                        format!(
                            "Found {} concepts related to: {}.",
                            count, concept_list
                        )
                    }
                }
            }
            _ => self.realize_no_answer(language),
        }
    }

    fn realize_refutation(&self, query: &Formula, language: DetectedLanguage) -> String {
        match (query, language) {
            (Formula::Atom(pred, args), DetectedLanguage::Vietnamese) if args.len() == 1 => {
                format!("{} KHÔNG phải là {}.", args[0], pred.to_lowercase())
            }
            (Formula::Atom(pred, args), _) if args.len() == 1 => {
                format!("{} is NOT {}.", args[0], pred.to_lowercase())
            }
            _ => self.realize_no_answer(language),
        }
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

    #[test]
    fn detects_probabilistic_keywords_en() {
        let p = NlpParser::new();
        // Use "probably" keyword which triggers probabilistic
        let msg = p.parse_message("what is probably rain?", DetectedLanguage::English).unwrap();
        assert_eq!(msg.query_type, QueryType::Probabilistic);
    }

    #[test]
    fn detects_probabilistic_keywords_vi() {
        let p = NlpParser::new();
        let msg = p.parse_message("có lẽ mưa là gì?", DetectedLanguage::Vietnamese).unwrap();
        assert_eq!(msg.query_type, QueryType::Probabilistic);
    }

    #[test]
    fn detects_causal_keywords_en() {
        let p = NlpParser::new();
        let msg = p.parse_message("why rain wet grass?", DetectedLanguage::English).unwrap();
        assert_eq!(msg.query_type, QueryType::Causal);
    }

    #[test]
    fn detects_causal_keywords_vi() {
        let p = NlpParser::new();
        let msg = p.parse_message("tại sao mưa ướt cỏ?", DetectedLanguage::Vietnamese).unwrap();
        assert_eq!(msg.query_type, QueryType::Causal);
    }

    #[test]
    fn detects_world_keywords_en() {
        let p = NlpParser::new();
        let msg = p.parse_message("what is agent population?", DetectedLanguage::English).unwrap();
        assert_eq!(msg.query_type, QueryType::World);
    }

    #[test]
    fn detects_world_keywords_vi() {
        let p = NlpParser::new();
        let msg = p.parse_message("từ vựng agent là gì?", DetectedLanguage::Vietnamese).unwrap();
        assert_eq!(msg.query_type, QueryType::World);
    }

    #[test]
    fn detects_knowledge_graph_keywords_en() {
        let p = NlpParser::new();
        let msg = p.parse_message("what is related bird?", DetectedLanguage::English).unwrap();
        assert_eq!(msg.query_type, QueryType::KnowledgeGraph);
    }

    #[test]
    fn detects_knowledge_graph_keywords_vi() {
        let p = NlpParser::new();
        let msg = p.parse_message("liên quan chim là gì?", DetectedLanguage::Vietnamese).unwrap();
        assert_eq!(msg.query_type, QueryType::KnowledgeGraph);
    }

    #[test]
    fn default_query_type_is_logical() {
        let p = NlpParser::new();
        let msg = p.parse_message("what is human?", DetectedLanguage::English).unwrap();
        assert_eq!(msg.query_type, QueryType::Logical);
    }
}
