use omiai_io::{ChatEngine, ChatRequest, conversation::ConversationMemory, nlp_parser::NlpParser};

fn main() {
    let parser = NlpParser::new();
    
    // Test what the parser produces for each input
    let tests = vec![
        "every human is mortal",
        "socrates is human", 
        "is socrates mortal",
        "socrates mortal?",
    ];
    
    for text in tests {
        let lang = parser.detect_language(text).unwrap_or(omiai_io::nlp_parser::DetectedLanguage::English);
        println!("\nInput: '{}' (lang: {:?})", text, lang);
        match parser.parse_message(text, lang) {
            Ok(parsed) => {
                println!("  Intent: {:?}", parsed.intent);
                println!("  Formula: {:?}", parsed.formula);
                println!("  Query: {:?}", parsed.query);
                println!("  QueryType: {:?}", parsed.query_type);
            }
            Err(e) => println!("  Parse error: {}", e),
        }
    }
}
