// src/main.rs
use crate::tag::TagPath;
use pest::Parser;
use pest_derive::Parser;
use std::fmt;

#[derive(Parser)]
#[grammar = "ast.pest"]
struct ASTParser;

#[derive(Debug)]
pub enum AST {
    AND(Box<AST>, Box<AST>),
    OR(Box<AST>, Box<AST>),
    NOT(Box<AST>),
    VAL(TagPath),
}

impl fmt::Display for AST {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AST::AND(lhs, rhs) => write!(f, "({} AND {})", lhs, rhs),
            AST::OR(lhs, rhs) => write!(f, "({} OR {})", lhs, rhs),
            AST::NOT(expr) => write!(f, "(NOT {})", expr),
            AST::VAL(p) => write!(f, "name = \'{}\'", p.segments().join("/")),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

impl AST {
    pub fn parse_query(ts: &str) -> Result<AST, QueryError> {
        let parsed = ASTParser::parse(Rule::program, ts)
            .map_err(|e| QueryError::InvalidQuery(e.to_string()))?
            .next()
            .ok_or_else(|| QueryError::InvalidQuery(String::from("empty query")))?;

        Ok(Self::generate_ast(parsed))
    }

    pub fn generate_ast(pair: pest::iterators::Pair<Rule>) -> AST {
        match pair.as_rule() {
            Rule::program | Rule::expression | Rule::or_expr => {
                let mut inner = pair.into_inner();
                let mut ast = Self::generate_ast(inner.next().unwrap());

                while let Some(op_pair) = inner.next() {
                    match op_pair.as_rule() {
                        Rule::OR => {
                            let rhs = Self::generate_ast(inner.next().unwrap());
                            ast = AST::OR(Box::new(ast), Box::new(rhs));
                        }
                        Rule::EOI => {}
                        _ => panic!(),
                    }
                }

                ast
            }
            Rule::and_expr => {
                let mut inner = pair.into_inner();
                let mut ast = Self::generate_ast(inner.next().unwrap());

                while let Some(op_pair) = inner.next() {
                    assert_eq!(op_pair.as_rule(), Rule::AND);
                    let rhs = Self::generate_ast(inner.next().unwrap());
                    ast = AST::AND(Box::new(ast), Box::new(rhs));
                }

                ast
            }
            Rule::not_expr => {
                let mut inner = pair.into_inner();
                let first = inner.next().unwrap();
                match first.as_rule() {
                    Rule::NOT => {
                        let operand = Self::generate_ast(inner.next().unwrap());
                        AST::NOT(Box::new(operand))
                    }
                    _ => Self::generate_ast(first),
                }
            }
            Rule::primary => Self::generate_ast(pair.into_inner().next().unwrap()),
            Rule::tag_path => AST::VAL(
                TagPath::parse(pair.as_str()).expect("grammar only admits valid tag paths"),
            ),
            _ => unreachable!("Unexpected rule: {:?}", pair.as_rule()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let input = "a AND b AND c d";
        let parsed = ASTParser::parse(Rule::program, input);

        assert!(parsed.is_err(), "invalid parse succeeded");
        let input = "a AND (NOT (b OR c))";
        let ast = AST::parse_query(input);
        assert_eq!(
            format!("{}", ast.unwrap()),
            "(name = \'a\' AND (NOT (name = \'b\' OR name = \'c\')))"
        );

        let input = "a AND b AND c AND d";
        let ast = AST::parse_query(input);
        assert_eq!(
            format!("{}", ast.unwrap()),
            "(((name = \'a\' AND name = \'b\') AND name = \'c\') AND name = \'d\')"
        );

        let ast = AST::parse_query("root/mid AND NOT 你好/世界")?;
        assert_eq!(
            format!("{}", ast),
            "(name = \'root/mid\' AND (NOT name = \'你好/世界\'))"
        );
        match AST::parse_query("root/mid")? {
            AST::VAL(p) => assert_eq!(p.segments(), ["root", "mid"]),
            other => panic!("expected a single tag path, got {other:?}"),
        }
        for bad in ["root / mid", "root//mid", "/root", "root/", "a AND", ""] {
            assert!(AST::parse_query(bad).is_err(), "{bad:?} should not parse");
        }

        Ok(())
    }
}
