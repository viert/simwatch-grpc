use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
  Illegal,
  EOF,

  Ident,
  Integer,
  Float,
  String,

  NotEquals,
  Equals,
  Matches,
  NotMatches,
  Less,
  Greater,
  LessOrEqual,
  GreaterOrEqual,

  LeftBrace,
  RightBrace,

  Or,
  And,
}

impl Display for TokenKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TokenKind::Illegal => write!(f, "Illegal"),
      TokenKind::EOF => write!(f, "EOF"),
      TokenKind::Ident => write!(f, "Ident"),
      TokenKind::Integer => write!(f, "Integer"),
      TokenKind::Float => write!(f, "Float"),
      TokenKind::String => write!(f, "String"),
      TokenKind::NotEquals => write!(f, "NotEquals"),
      TokenKind::Equals => write!(f, "Equals"),
      TokenKind::Matches => write!(f, "Matches"),
      TokenKind::NotMatches => write!(f, "NotMatches"),
      TokenKind::Less => write!(f, "Less"),
      TokenKind::Greater => write!(f, "Greater"),
      TokenKind::LessOrEqual => write!(f, "LessOrEqual"),
      TokenKind::GreaterOrEqual => write!(f, "GreaterOrEqual"),
      TokenKind::LeftBrace => write!(f, "LeftBrace"),
      TokenKind::RightBrace => write!(f, "RightBrace"),
      TokenKind::Or => write!(f, "Or"),
      TokenKind::And => write!(f, "And"),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Token {
  pub src: String,
  pub kind: TokenKind,
  pub line: usize,
  pub pos: usize,
}

pub struct TokenFlow<'a> {
  tokens: &'a Vec<Token>,
  idx: usize,
}

impl<'a> TokenFlow<'a> {
  pub fn new(tokens: &'a Vec<Token>) -> Self {
    Self { tokens, idx: 0 }
  }

  fn get(&self, idx: usize) -> Option<&'a Token> {
    if idx < self.tokens.len() {
      self.tokens.get(idx)
    } else {
      None
    }
  }

  pub fn current(&self) -> Option<&'a Token> {
    self.get(self.idx)
  }

  pub fn next(&self) -> Option<&'a Token> {
    self.get(self.idx + 1)
  }

  pub fn advance(&mut self) {
    if self.idx < self.tokens.len() {
      self.idx += 1
    }
  }

  pub fn reset(&mut self) {
    self.idx = 0
  }
}
