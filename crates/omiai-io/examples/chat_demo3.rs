use omiai_io::{ChatEngine, ChatRequest, nlp_parser::DetectedLanguage, conversation::ConversationMemory};
use omiai_io::dialogue_router::DialogueRouter;
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_core::logic_engine::{Formula, Term};

fn main() {
    // Create router with knowledge
    let mut router = DialogueRouter::new();
    
    // Add concepts
    let human = Concept { id: "Human".into(), label: "Human".into() };
    let mortal = Concept { id: "Mortal".into(), label: "Mortal".into() };
    router.add_concept(human);
    router.add_concept(mortal);
    
    // Add Socrates
    router.add_concept(Concept { id: "Socrates".into(), label: "Socrates".into() });
    router.add_relation("Socrates", "Human", "InstanceOf").unwrap();
    router.add_relation("Human", "Mortal", "implies").unwrap();
    
    // Create engine with this router
    let mut engine = ChatEngine::new();
    engine.set_router(router);
    let mut memory = ConversationMemory::default();
    
    // Test with proper syntax the parser understands
    let tests = vec![
        "hello",
        "xin chào",
        "every human is mortal",     // Assert universal rule
        "socrates is human",         // Assert fact
        "socrates mortal?",          // Query: Is Socrates mortal?
        "why socrates mortal",       // Explain why (causal)
        "probably rain",             // Probabilistic
        "có lẽ mưa",                 // Probabilistic Vietnamese
    ];
    
    println!("=== OmiAI Chat Demo with Knowledge ===\n");
    for text in tests {
        let response = engine.handle(
            &ChatRequest { text: text.into(), preferred_language: None },
            &mut memory,
        );
        println!("User: {}", text);
        println!("Bot ({:?}): {}\n", response.language, response.text);
    }
}
