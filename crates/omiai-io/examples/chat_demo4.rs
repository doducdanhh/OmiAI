use omiai_io::{ChatEngine, ChatRequest, conversation::ConversationMemory};
use omiai_io::dialogue_router::DialogueRouter;
use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_core::logic_engine::{Formula, Term};
use omiai_core::prover::TheoremProver;

fn main() {
    // First, test the core prover directly
    let mut prover = TheoremProver::new();
    
    // Premises: ∀x (Human(x) → Mortal(x)), Human(Socrates)
    let premises = vec![
        Formula::ForAll(
            "x".into(),
            Box::new(Formula::Implies(
                Box::new(Formula::atom("Human", vec![Term::Var("x".into())])),
                Box::new(Formula::atom("Mortal", vec![Term::Var("x".into())])),
            )),
        ),
        Formula::atom("Human", vec![Term::Const("Socrates".into())]),
    ];
    
    // Query: Mortal(Socrates)
    let query = Formula::atom("Mortal", vec![Term::Const("Socrates".into())]);
    let proof = prover.prove(&premises, &query);
    println!("Direct prover test: {:?}", proof);
    
    // Now test through chat engine with proper memory
    let mut router = DialogueRouter::new();
    let mut engine = ChatEngine::new();
    engine.set_router(router);
    let mut memory = ConversationMemory::default();
    
    // The key is to use Assert intent to add to memory first
    let tests = vec![
        // Assert the rule
        ("every human is mortal", "Assert universal rule"),
        // Assert the fact  
        ("socrates is human", "Assert Socrates is human"),
        // Now query - should find proof from memory
        ("is socrates mortal", "Query Socrates mortal"),
    ];
    
    println!("\n=== Chat Engine Test ===\n");
    for (text, desc) in tests {
        let response = engine.handle(
            &ChatRequest { text: text.into(), preferred_language: None },
            &mut memory,
        );
        println!("{}", desc);
        println!("User: {}", text);
        println!("Bot ({:?}): {}\n", response.language, response.text);
    }
}
