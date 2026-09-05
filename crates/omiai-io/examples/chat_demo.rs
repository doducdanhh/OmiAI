use omiai_io::{ChatEngine, ChatRequest, nlp_parser::DetectedLanguage, conversation::ConversationMemory};

fn main() {
    let mut engine = ChatEngine::new();
    let mut memory = ConversationMemory::default();
    
    // Test cases
    let tests = vec![
        "hello",
        "xin chào",
        "human mortal",
        "người phàm",
        "why human mortal",
        "tại sao người phàm",
        "probably rain",
        "có lẽ mưa",
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
