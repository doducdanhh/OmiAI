use omiai_io::tokenizer::tokenize;

fn main() {
    let tests = vec![
        "is socrates mortal?",
        "is socrates mortal",
        "what is mortal?",
    ];
    
    for text in tests {
        println!("\nInput: '{}'", text);
        match tokenize(text) {
            Ok(tokens) => {
                println!("  Tokens: {:?}", tokens);
                let words: Vec<String> = tokens
                    .iter()
                    .filter_map(|t| match t {
                        omiai_io::tokenizer::Token::Ident(s) | omiai_io::tokenizer::Token::StringLit(s) => Some(s.to_lowercase()),
                        _ => None,
                    })
                    .collect();
                println!("  Words: {:?}", words);
            }
            Err(e) => println!("  Error: {}", e),
        }
    }
}
