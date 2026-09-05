//! Dialogue front-end: turns raw user text into structured dialogue,
//! routes it through the symbolic core, and realizes answers back into
//! English or Vietnamese.
//!
//! # Modules
//! - [`nlp_parser`] — multilingual intent parsing.
//! - [`tokenizer`] — word tokenizer.
//! - [`perception`] — input perception utilities.
//! - [`action`] — actionable intents.
//! - [`chat`] — chat engine on top of the prover.
//! - [`conversation`] — dialogue memory (moved from `meta::introspection`,
//!   ADR-0005).
//! - [`dialogue_router`] — routes parsed intents to all 8 reasoning pillars.

#![allow(dead_code)]

pub mod action;
pub mod chat;
pub mod conversation;
pub mod dialogue_router;
pub mod nlp_parser;
pub mod perception;
pub mod tokenizer;

pub use crate::chat::{ChatEngine, ChatRequest, ChatResponse};
pub use crate::conversation::ConversationMemory;
pub use crate::dialogue_router::{CausalExplanation, CausalQuery, DialogueRouter, DialogueRouterConfig, KnowledgeQuery, KnowledgeResult, ProbMethod, ReasoningResult};
pub use crate::nlp_parser::{DetectedLanguage, NlpParser, ParseIntent};
