use omiai_io::{ChatEngine, ChatRequest, conversation::ConversationMemory};
use omiai_core::logic_engine::{Formula, Term};
use omiai_core::prover::TheoremProver;

fn main() {
    // First verify the memory facts work with the prover
    let mut memory = ConversationMemory::default();
    
    // Push facts manually
    memory.push_fact(Formula::ForAll(
        "x".into(),
        Box::new(Formula::Implies(
            Box::new(Formula::atom("Human", vec![Term::Var("x".into())])),
            Box::new(Formula::atom("Mortal", vec![Term::Var("x".into())])),
        )),
    ));
    memory.push_fact(Formula::atom("Human", vec![Term::Const("Socrates".into())]));
    
    let facts = memory.facts();
    println!("Memory facts: {:?}", facts);
    
    let query = Formula::atom("Mortal", vec![Term::Const("Socrates".into())]);
    let proof = TheoremProver::new().prove(&facts, &query);
    println!("Proof result: {:?}", proof);
}
