use crate::lee::lexer::token::{Token, TokenKind};
use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum ParseError {
  UnexpectedToken(Token),
  UnexpectedTokenType(Token, Vec<TokenKind>),
  UnexpectedEOF(Token),
  UnexpectedEOS(Vec<TokenKind>),
  ConvertError(Token, Box<dyn Error>),
  InvalidValueType(Token, Vec<&'static str>),
}

impl Display for ParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ParseError::UnexpectedToken(t) => write!(
        f,
        "unexpected token {}({}) at line={} pos={}",
        t.kind, t.src, t.line, t.pos
      ),
      ParseError::UnexpectedTokenType(t, exp) => {
        let types: Vec<String> = exp.iter().map(|k| k.to_string()).collect();
        write!(
          f,
          "unexpected token type {}({}) at line={} pos={}, expected one of [{}]",
          t.kind,
          t.src,
          t.line,
          t.pos,
          types.join(", ")
        )
      }
      ParseError::UnexpectedEOF(t) => write!(f, "unexpected EOF at line={} pos={}", t.line, t.pos),
      ParseError::UnexpectedEOS(exp) => {
        let types: Vec<String> = exp.iter().map(|k| k.to_string()).collect();
        write!(
          f,
          "unexpected end of stream while waiting for one of [{}]",
          types.join(", ")
        )
      }
      ParseError::ConvertError(t, err) => {
        write!(f, "error while converting {}: {}", t.kind, err)
      }
      ParseError::InvalidValueType(t, exp) => {
        write!(
          f,
          "invalid value type at line={} pos={}, expected one of [{}]",
          t.line,
          t.pos,
          exp.join(", ")
        )
      }
    }
  }
}

pub struct CompileError {
  pub msg: String,
}
impl Display for CompileError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "compilation error: {}", self.msg)
  }
}
