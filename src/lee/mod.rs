/// Logical expression evaluator
///
use self::{
  lexer::Lexer,
  parser::{error::ParseError, expression::Expression, parse},
};

pub mod lexer;
pub mod parser;

pub fn make_expr<T>(query: &str) -> Result<Expression<T>, ParseError> {
  let mut l = Lexer::new(query);
  let mut tf = l.parse();
  parse(&mut tf)
}
