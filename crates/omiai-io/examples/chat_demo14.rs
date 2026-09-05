use omiai_io::{ChatEngine, ChatRequest, conversation::ConversationMemory};

fn main() {
    let mut engine = ChatEngine::new();
    let mut memory = ConversationMemory::default();
    
    // Comprehensive test cases
    let tests = vec![
        // Greetings
        "hello",
        "xin chào",
        
        // Logical assertions
        "every human is mortal",
        "every bird can fly",
        "socrates is human",
        "tweety is bird",
        
        // Logical queries (with question mark)
        "is socrates mortal?",
        "is tweety fly?",
        "is socrates human?",
        
        // Probabilistic
        "probably rain",
        "có lẽ mưa",
        
        // Causal (why questions)
        "why socrates mortal?",
        "tại sao người phàm?",
    ];
    
    println!("=== OmiAI Comprehensive Chat Demo ===\n");
    for text in tests {
        let response = engine.handle(
            &ChatRequest { text: text.into(), preferred_language: None },
            &mut memory,
        );
        println!("User: {}", text);
        println!("Bot ({:?}): {}", response.language, response.text);
        println!("  [Intent: {:?}, Proven: {}, Confidence: {}]", response.intent, response.proven, response.confidence);
        println!();
    }
}
