use omiai_io::{ChatEngine, ChatRequest, conversation::ConversationMemory};
use omiai_io::dialogue_router::DialogueRouter;
use omiai_probabilistic::bayesian::{BayesianNetwork, Cpt};

fn main() {
    // Create router with Bayesian network
    let mut router = DialogueRouter::new();
    
    // Add a simple Bayesian network: Rain -> WetGrass
    let mut bn = BayesianNetwork::new();
    bn.add_node(Cpt {
        variable: "Rain".into(),
        parents: vec![],
        probs_true: vec![0.2], // P(Rain) = 0.2
    });
    bn.add_node(Cpt {
        variable: "WetGrass".into(),
        parents: vec!["Rain".into()],
        probs_true: vec![0.9, 0.2], // P(WetGrass|Rain), P(WetGrass|!Rain)
    });
    router.add_bayesian_network(bn);
    
    // Create engine with this router
    let mut engine = ChatEngine::new();
    engine.set_router(router);
    let mut memory = ConversationMemory::default();
    
    let tests = vec![
        "what is the probability of rain",
        "how likely is rain",
    ];
    
    println!("=== OmiAI Probabilistic Demo ===\n");
    for text in tests {
        let response = engine.handle(
            &ChatRequest { text: text.into(), preferred_language: None },
            &mut memory,
        );
        println!("User: {}", text);
        println!("Bot ({:?}): {}\n", response.language, response.text);
    }
}
