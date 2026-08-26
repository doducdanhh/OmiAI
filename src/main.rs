//! OmiAI command-line runtime for chat and guarded continual learning.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use omiai::io::nlp_parser::NlpParser;
use omiai::memory::episodic::{Episode, EpisodeSource};
use omiai::meta::continual_learning::ContinualLearningEngine;
use omiai::persistence::{load_checkpoint, save_checkpoint};
use omiai::{ChatEngine, ChatRequest, ConversationMemory};

#[derive(Debug, Parser)]
#[command(
    name = "omiai",
    about = "Symbolic, zero-training continual intelligence runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start an interactive multilingual chat session.
    Chat,
    /// Observe text and retain its parsed assertion as an episode.
    Learn {
        text: String,
        #[arg(long, default_value = "data/learning_state.json")]
        state: PathBuf,
        #[arg(long, default_value_t = 0.75)]
        confidence: f64,
    },
    /// Consolidate repeated observations into trusted symbolic knowledge.
    Consolidate {
        #[arg(long, default_value = "data/learning_state.json")]
        state: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    match Cli::parse().command {
        Command::Chat => run_chat(),
        Command::Learn {
            text,
            state,
            confidence,
        } => learn(text, state, confidence),
        Command::Consolidate { state } => consolidate(state),
    }
}

fn run_chat() -> anyhow::Result<()> {
    let engine = ChatEngine::new();
    let mut memory = ConversationMemory::default();
    let stdin = io::stdin();
    println!("OmiAI chat. Enter /quit to stop.");
    loop {
        print!("> ");
        io::stdout().flush().context("failed to flush prompt")?;
        let mut line = String::new();
        if stdin.read_line(&mut line).context("failed to read input")? == 0 {
            break;
        }
        let text = line.trim();
        if text.eq_ignore_ascii_case("/quit") {
            break;
        }
        if text.is_empty() {
            continue;
        }
        let response = engine.handle(
            &ChatRequest {
                text: text.into(),
                preferred_language: None,
            },
            &mut memory,
        );
        println!("{}", response.text);
    }
    Ok(())
}

fn learn(text: String, state: PathBuf, confidence: f64) -> anyhow::Result<()> {
    let mut learner: ContinualLearningEngine = if state.exists() {
        load_checkpoint(&state).context("failed to load learning state")?
    } else {
        ContinualLearningEngine::default()
    };
    let parser = NlpParser::new();
    let language = parser
        .detect_language(&text)
        .unwrap_or(omiai::DetectedLanguage::English);
    let parsed = parser
        .parse_message(&text, language)
        .map_err(anyhow::Error::msg)?;
    let episode = Episode::new(EpisodeSource::User, &text, parsed.formula, confidence)
        .with_language(language);
    learner.observe(episode);
    save_checkpoint(&state, &learner).context("failed to checkpoint learning state")?;
    println!("Observation stored for guarded consolidation.");
    Ok(())
}

fn consolidate(state: PathBuf) -> anyhow::Result<()> {
    let mut learner: ContinualLearningEngine =
        load_checkpoint(&state).context("failed to load learning state")?;
    let report = learner.consolidate();
    save_checkpoint(&state, &learner).context("failed to checkpoint consolidated state")?;
    println!(
        "promoted={}, quarantined={}, already_known={}",
        report.promoted, report.quarantined, report.already_known
    );
    Ok(())
}
