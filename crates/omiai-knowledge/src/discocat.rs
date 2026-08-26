//! Distributional Compositional Categorial (DisCoCat) framework.
//!
//! DisCoCat (Coecke, Sadrzadeh, Clark 2010) lifts categorial grammar to
//! a **compact closed category** (pregroup grammar) and equips words with
//! vector-space semantics, so that grammatical reduction corresponds to
//! tensor contraction. The result is a single sentence vector that can
//! be compared for similarity, entailment, or classification.
//!
//! # What this module provides
//!
//! - A simple **pregroup grammar** with type reduction by squaring
//!   adjoints (`a^l · a · a^r ⇒ ε`).
//! - A **lexicon** that maps words to `(pregroup type, vector)` pairs.
//! - A **tensor reduction** engine that contracts a sentence's word
//!   tensors along the adjoints, yielding a sentence-level vector.
//! - A **bilinear map** for sentence-vs-sentence similarity (matching the
//!   categorial type of the sentence).
//!
//! # References
//!
//! - Coecke, Sadrzadeh, Clark, *Mathematical Foundations for a
//!   Compositional Distributional Model of Meaning* (2010).
//! - Lambek, *From word to sentence* (2008, CSLI).
//! - Kartsaklis, Sadrzadeh, *A compositional distributional inclusion
//!   model for sentence similarity* (2014).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pregroup types
// ---------------------------------------------------------------------------

/// A pregroup type: a sequence of **atoms** (each tagged as the atom
/// itself, its left adjoint `Adjoint::L`, or its right adjoint
/// `Adjoint::R`).
///
/// `n` (noun), `s` (sentence), `n^l`, `s^r`, … are all atoms with
/// different polarities.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Atom {
    pub name: String,
    pub adjoint: Adjoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Adjoint {
    None,
    L, // left adjoint: a^l · a ⇒ ε
    R, // right adjoint: a · a^r ⇒ ε
}

impl Atom {
    pub fn n() -> Self {
        Self {
            name: "n".into(),
            adjoint: Adjoint::None,
        }
    }
    pub fn s() -> Self {
        Self {
            name: "s".into(),
            adjoint: Adjoint::None,
        }
    }
    pub fn adjoint(self, side: Adjoint) -> Self {
        Self {
            name: self.name,
            adjoint: side,
        }
    }
    /// Cancel the pairing `a^l · a` or `a · a^r`. Returns `true` if the
    /// pair cancels (and removes them), `false` if not.
    pub fn cancels_with(&self, other: &Atom) -> bool {
        match (self.adjoint, other.adjoint) {
            (Adjoint::L, Adjoint::None) => self.name == other.name,
            (Adjoint::None, Adjoint::R) => self.name == other.name,
            _ => false,
        }
    }
}

/// A pregroup type: an ordered list of atoms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PregroupType(pub Vec<Atom>);

impl PregroupType {
    pub fn from_atoms(atoms: Vec<Atom>) -> Self {
        Self(atoms)
    }

    /// Noun phrase: `n`.
    pub fn np() -> Self {
        Self(vec![Atom::n()])
    }

    /// Sentence: `s`.
    pub fn sentence() -> Self {
        Self(vec![Atom::s()])
    }

    /// Transitive verb: `n^r · s · n^l`  (takes a noun on the right, leaves
    /// a noun slot on the left).
    pub fn transitive_verb() -> Self {
        Self(vec![
            Atom::n().adjoint(Adjoint::R),
            Atom::s(),
            Atom::n().adjoint(Adjoint::L),
        ])
    }

    /// Reduce this type to a canonical form by cancelling adjoint pairs
    /// greedily. Returns the residual type after reduction.
    pub fn reduce(&self) -> PregroupType {
        let mut stack = self.0.clone();
        let mut changed = true;
        while changed {
            changed = false;
            let mut i = 0;
            while i + 1 < stack.len() {
                if stack[i].cancels_with(&stack[i + 1]) {
                    stack.remove(i);
                    stack.remove(i);
                    changed = true;
                } else {
                    i += 1;
                }
            }
        }
        PregroupType(stack)
    }

    /// True iff the type reduces to the empty type `ε` (the unit).
    pub fn is_well_typed(&self) -> bool {
        self.reduce().0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Lexicon: word → (type, tensor)
// ---------------------------------------------------------------------------

/// A lexical entry: a word with its pregroup type and a word embedding
/// (for nouns, adjectives, …) or a relation tensor (for verbs, ditransitives).
#[derive(Debug, Clone)]
pub struct LexEntry {
    pub word: String,
    pub ptype: PregroupType,
    /// The semantic tensor:
    /// - For a noun `n`, a 1-D vector of dimension `dim`.
    /// - For a transitive verb `n^r · s · n^l`, a 3-D tensor of shape
    ///   `(dim, dim, dim)`.
    pub tensor: Vec<Vec<Vec<f64>>>,
    pub dim: usize,
}

impl LexEntry {
    /// Build a noun entry from a 1-D vector (stored as 3-D shape `(dim,1,1)`).
    pub fn noun(word: impl Into<String>, vec: Vec<f64>) -> Self {
        let dim = vec.len();
        // Build tensor to encode the vector as a 3-D structure
        // (shape (dim, 1, 1)).
        let mut outer = Vec::with_capacity(dim);
        for &v in &vec {
            outer.push(vec![vec![v]]);
        }
        Self {
            word: word.into(),
            ptype: PregroupType::np(),
            tensor: outer,
            dim,
        }
    }

    /// Build a transitive verb entry from a 3-D tensor.
    pub fn transitive_verb(word: impl Into<String>, tensor: Vec<Vec<Vec<f64>>>) -> Self {
        let dim = tensor.len();
        Self {
            word: word.into(),
            ptype: PregroupType::transitive_verb(),
            tensor,
            dim,
        }
    }
}

/// A lexicon: word → entry.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    pub entries: HashMap<String, LexEntry>,
    pub dim: usize,
}

impl Lexicon {
    pub fn new(dim: usize) -> Self {
        Self {
            entries: HashMap::new(),
            dim,
        }
    }

    pub fn insert(&mut self, entry: LexEntry) {
        self.entries.insert(entry.word.clone(), entry);
    }

    pub fn get(&self, word: &str) -> Option<&LexEntry> {
        self.entries.get(word)
    }
}

// ---------------------------------------------------------------------------
// Tensor reduction (simplified for noun-verb-noun)
// ---------------------------------------------------------------------------

/// A simplified DisCoCat reduction for sentences of the form
/// `NP · TV · NP` (subject, transitive verb, object).
///
/// This restricts to 3-word transitive sentences, where the tensors
/// can be contracted explicitly:
/// - NP: vector of length `dim` (n slot)
/// - TV: 3-D tensor `(dim, dim, dim)` for `(object, subject, sentence)`
/// - NP: vector of length `dim` (n slot)
///
/// The contraction is:
///   `v[o] · T[o, s, k] · u[s]` summed over `s, o` to yield a vector of length `dim`.
pub fn reduce_transitive(np_subj: &LexEntry, tv: &LexEntry, np_obj: &LexEntry) -> Vec<f64> {
    let dim = tv.dim;
    assert_eq!(np_subj.dim, dim, "subject noun dim mismatch");
    assert_eq!(np_obj.dim, dim, "object noun dim mismatch");
    let mut result = vec![0.0f64; dim];
    // result[k] = sum_{o, s} T[o][s][k] * np_obj[o] * np_subj[s]
    for (k, r_k) in result.iter_mut().enumerate() {
        let mut acc = 0.0;
        for o in 0..dim {
            for s in 0..dim {
                let t_val = tv
                    .tensor
                    .get(o)
                    .and_then(|x| x.get(s))
                    .and_then(|x| x.get(k))
                    .copied()
                    .unwrap_or(0.0);
                let subj_val = np_subj
                    .tensor
                    .get(s)
                    .and_then(|x| x.first())
                    .and_then(|x| x.first())
                    .copied()
                    .unwrap_or(0.0);
                let obj_val = np_obj
                    .tensor
                    .get(o)
                    .and_then(|x| x.first())
                    .and_then(|x| x.first())
                    .copied()
                    .unwrap_or(0.0);
                acc += t_val * subj_val * obj_val;
            }
        }
        *r_k = acc;
    }
    result
}

// ---------------------------------------------------------------------------
// Sentence similarity (bilinear map on s-typed vectors)
// ---------------------------------------------------------------------------

/// Bilinear map for sentence-sentence similarity.
///
/// Following Kartsaklis & Sadrzadeh (2014), a sentence of type `s` is
/// represented as a 3-D tensor `F[i,j,k]` so that inner product with
/// `F[j,k,l]` and a verb-like tensor yields an `s`-typed vector. For
/// similarity we use a simple bilinear form.
///
/// This implementation provides a pragmatic alternative: cosine
/// similarity between sentence vectors projected through a learned
/// bilinear map `B` (here: identity or diagonal scaling).
pub fn sentence_similarity(a: &[f64], b: &[f64], bilinear: &[Vec<f64>]) -> f64 {
    if a.len() != b.len() || bilinear.len() != a.len() {
        return 0.0;
    }
    // Compute B(a) · B(b)
    let ba: Vec<f64> = (0..a.len())
        .map(|i| bilinear[i].iter().zip(a.iter()).map(|(b, x)| b * x).sum())
        .collect();
    let bb: Vec<f64> = (0..b.len())
        .map(|i| bilinear[i].iter().zip(b.iter()).map(|(b, x)| b * x).sum())
        .collect();
    let dot: f64 = ba.iter().zip(bb.iter()).map(|(x, y)| x * y).sum();
    let na = ba.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb = bb.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na < 1e-12 || nb < 1e-12 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Cosine similarity between two vectors.
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na < 1e-12 || nb < 1e-12 {
        0.0
    } else {
        dot / (na * nb)
    }
}

// ---------------------------------------------------------------------------
// Default lexicon and parser
// ---------------------------------------------------------------------------

/// Build a small toy lexicon with a few nouns and verbs.
pub fn toy_lexicon() -> Lexicon {
    let dim = 4;
    let mut lex = Lexicon::new(dim);
    lex.insert(LexEntry::noun("alice", vec![0.9, 0.1, 0.0, 0.2]));
    lex.insert(LexEntry::noun("bob", vec![0.1, 0.9, 0.2, 0.0]));
    lex.insert(LexEntry::noun("cat", vec![0.0, 0.2, 0.9, 0.1]));
    lex.insert(LexEntry::noun("dog", vec![0.2, 0.0, 0.1, 0.9]));
    // Verb "likes": random tensor
    let mut likes = vec![vec![vec![0.0; dim]; dim]; dim];
    for (i, plane) in likes.iter_mut().enumerate() {
        for (j, row) in plane.iter_mut().enumerate() {
            for (k, cell) in row.iter_mut().enumerate() {
                *cell = ((i + j + k) as f64 * 0.07).sin();
            }
        }
    }
    lex.insert(LexEntry::transitive_verb("likes", likes));
    let mut sees = vec![vec![vec![0.0; dim]; dim]; dim];
    for (i, plane) in sees.iter_mut().enumerate() {
        for (j, row) in plane.iter_mut().enumerate() {
            for (k, cell) in row.iter_mut().enumerate() {
                *cell = ((i * 2 + j * 3 + k) as f64 * 0.05).cos();
            }
        }
    }
    lex.insert(LexEntry::transitive_verb("sees", sees));
    lex
}

/// Parse a simple 3-word `NP V NP` sentence and return its vector, or
/// `None` if any word is missing.
pub fn parse_transitive(lex: &Lexicon, subj: &str, verb: &str, obj: &str) -> Option<Vec<f64>> {
    let s = lex.get(subj)?;
    let v = lex.get(verb)?;
    let o = lex.get(obj)?;
    // Type check: subject and object must be `n`, verb must be `n^r s n^l`.
    if s.ptype != PregroupType::np() {
        return None;
    }
    if v.ptype != PregroupType::transitive_verb() {
        return None;
    }
    if o.ptype != PregroupType::np() {
        return None;
    }
    // Combined type: n · n^r · s · n^l · n. Reduction yields s ✓.
    Some(reduce_transitive(s, v, o))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pregroup_transitive_verb_reduces_via_subj_obj() {
        // Subject n, verb n^r·s·n^l, object n concatenate to
        // n · n^r · s · n^l · n. The adjoint pairs cancel pairwise,
        // leaving exactly the sentence atom `s`.
        let mut ty = PregroupType::np().0;
        let mut verb = PregroupType::transitive_verb().0;
        let mut obj = PregroupType::np().0;
        ty.append(&mut verb);
        ty.append(&mut obj);
        let combined = PregroupType(ty).reduce();
        assert_eq!(
            combined,
            PregroupType::sentence(),
            "transitive sentence must reduce to s"
        );
    }

    #[test]
    fn lexicon_noun_creation() {
        let n = LexEntry::noun("alice", vec![1.0, 2.0, 3.0]);
        assert_eq!(n.dim, 3);
        assert_eq!(n.ptype, PregroupType::np());
    }

    #[test]
    fn toy_lexicon_has_basic_words() {
        let lex = toy_lexicon();
        assert!(lex.get("alice").is_some());
        assert!(lex.get("bob").is_some());
        assert!(lex.get("likes").is_some());
        assert!(lex.get("sees").is_some());
        assert!(lex.get("nonexistent").is_none());
    }

    #[test]
    fn transitive_parse_returns_vector() {
        let lex = toy_lexicon();
        let v = parse_transitive(&lex, "alice", "likes", "bob").expect("should parse");
        assert_eq!(v.len(), lex.dim);
        // Sentence vector should be non-zero
        assert!(v.iter().any(|x| x.abs() > 1e-9));
    }

    #[test]
    fn cosine_similarity_self_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_similarity_orthogonal_is_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine(&a, &b).abs() < 1e-9);
    }

    #[test]
    fn parse_returns_none_for_unknown_word() {
        let lex = toy_lexicon();
        assert!(parse_transitive(&lex, "alice", "likes", "nobody").is_none());
    }

    #[test]
    fn sentence_similarity_matches_cosine_for_identity() {
        let v = vec![1.0, 2.0, 3.0];
        let id: Vec<Vec<f64>> = (0..3)
            .map(|i| {
                let mut row = vec![0.0; 3];
                row[i] = 1.0;
                row
            })
            .collect();
        let sim = sentence_similarity(&v, &v, &id);
        assert!((sim - 1.0).abs() < 1e-9);
    }
}
