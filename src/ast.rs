// src/main.rs
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
    VAL(String),
}

impl fmt::Display for AST {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AST::AND(lhs, rhs) => write!(f, "({} AND {})", lhs, rhs),
            AST::OR(lhs, rhs) => write!(f, "({} OR {})", lhs, rhs),
            AST::NOT(expr) => write!(f, "(NOT {})", expr),
            AST::VAL(s) => write!(f, "name = \'{}\'", s),
        }
    }
}

impl AST {
    pub fn parse_query(ts: &str) -> AST {
        let parsed = ASTParser::parse(Rule::program, ts)
            .expect("Parse failed")
            .next()
            .unwrap();

        Self::generate_ast(parsed)
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
            Rule::ident => AST::VAL(pair.as_str().to_string()),
            _ => unreachable!("Unexpected rule: {:?}", pair.as_rule()),
        }
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let input = "a AND b AND c d";
        let parsed = ASTParser::parse(Rule::program, input);

        assert!(parsed.is_err(), "invalid parse succeeded");
        let input = "a AND (NOT (b OR c))";
        let ast = super::AST::parse_query(input);
        assert_eq!(
            format!("{}", ast),
            "(name = \'a\' AND (NOT (name = \'b\' OR name = \'c\')))"
        );

        let input = "a AND b AND c AND d";
        let ast = super::AST::parse_query(input);
        assert_eq!(
            format!("{}", ast),
            "(((name = \'a\' AND name = \'b\') AND name = \'c\') AND name = \'d\')"
        );

        //TODO: add more tests with unicode characters

        Ok(())
    }
}
