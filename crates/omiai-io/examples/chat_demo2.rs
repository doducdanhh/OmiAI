use omiai_io::{ChatEngine, ChatRequest, nlp_parser::DetectedLanguage, conversation::ConversationMemory};
use omiai_io::dialogue_router::DialogueRouter;
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_core::logic_engine::{Formula, Term};

fn main() {
    // Create router with knowledge
    let mut router = DialogueRouter::new();
    
    // Add knowledge: Human(x) -> Mortal(x)
    let human = Concept { id: "Human".into(), label: "Human".into() };
    let mortal = Concept { id: "Mortal".into(), label: "Mortal".into() };
    router.add_concept(human);
    router.add_concept(mortal);
    router.add_relation("Human", "Mortal", "implies").unwrap();
    
    // Also add fact: Socrates is Human
    router.add_concept(Concept { id: "Socrates".into(), label: "Socrates".into() });
    router.add_relation("Socrates", "Human", "InstanceOf").unwrap();
    
    // Create engine with this router
    let mut engine = ChatEngine::new();
    engine.set_router(router);
    let mut memory = ConversationMemory::default();
    
    let tests = vec![
        "hello",
        "xin chào",
        "socrates human",     // Assert: Socrates is Human
        "socrates mortal",    // Query: Is Socrates mortal?
        "why socrates mortal", // Explain why
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
