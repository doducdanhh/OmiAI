use omiai_io::{ChatEngine, ChatRequest, conversation::ConversationMemory};

fn main() {
    let mut engine = ChatEngine::new();
    let mut memory = ConversationMemory::default();
    
    // Test with correct question format
    let tests = vec![
        "every human is mortal",
        "socrates is human",
        "is socrates mortal?",
    ];
    
    println!("=== OmiAI Chat Demo ===\n");
    for text in tests {
        let response = engine.handle(
            &ChatRequest { text: text.into(), preferred_language: None },
            &mut memory,
        );
        println!("User: {}", text);
        println!("Bot ({:?}): {}\n", response.language, response.text);
    }
}
