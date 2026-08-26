//! Rule-based tokenizer using `nom` combinators.

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, multispace0, satisfy},
    combinator::{map, recognize},
    multi::many0,
    sequence::{delimited, pair},
};

/// Token kinds produced by the tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Ident(String),
    Number(String),
    StringLit(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Colon,
    Question,
    Semicolon,
    Arrow,
    Op(String),
}

/// Tokenize an input string into a vector of [`Token`]s.
pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    match tokens(input) {
        Ok(("", ts)) => Ok(ts),
        Ok((rest, _)) => Err(format!("unparsed input near: {rest}")),
        Err(e) => Err(format!("tokenize error: {e}")),
    }
}

fn tokens(input: &str) -> IResult<&str, Vec<Token>> {
    many0(delimited(multispace0, token, multispace0))(input)
}

fn token(input: &str) -> IResult<&str, Token> {
    alt((
        map(tag("->"), |_| Token::Arrow),
        map(char('('), |_| Token::LParen),
        map(char(')'), |_| Token::RParen),
        map(char('['), |_| Token::LBracket),
        map(char(']'), |_| Token::RBracket),
        map(char(','), |_| Token::Comma),
        map(char('.'), |_| Token::Dot),
        map(char(':'), |_| Token::Colon),
        map(char('?'), |_| Token::Question),
        map(char(';'), |_| Token::Semicolon),
        map(string_lit, Token::StringLit),
        map(number, Token::Number),
        map(ident, Token::Ident),
        map(operator, Token::Op),
    ))(input)
}

fn ident(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            satisfy(|c: char| c.is_alphabetic() || c == '_'),
            many0(satisfy(|c: char| c.is_alphanumeric() || c == '_')),
        )),
        |s: &str| s.to_string(),
    )(input)
}

fn number(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            take_while1(|c: char| c.is_ascii_digit()),
            many0(satisfy(|c: char| c.is_ascii_digit() || c == '.')),
        )),
        |s: &str| s.to_string(),
    )(input)
}

fn string_lit(input: &str) -> IResult<&str, String> {
    delimited(
        char('"'),
        map(many0(satisfy(|c| c != '"')), |chars: Vec<char>| {
            chars.into_iter().collect()
        }),
        char('"'),
    )(input)
}

fn operator(input: &str) -> IResult<&str, String> {
    map(
        recognize(take_while1(|c: char| "+-*/<>=!&|".contains(c))),
        |s: &str| s.to_string(),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_simple_sentence() {
        let ts = tokenize("Human(socrates) -> Mortal(socrates)").unwrap();
        assert!(
            ts.iter()
                .any(|t| matches!(t, Token::Ident(s) if s == "Human"))
        );
        assert!(ts.iter().any(|t| matches!(t, Token::Arrow)));
    }
}
