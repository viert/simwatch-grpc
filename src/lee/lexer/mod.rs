pub mod string_reader;
pub mod token;

use lazy_static::lazy_static;
use regex::Regex;
use std::str::FromStr;
use string_reader::StringReader;
use token::{Token, TokenFlow, TokenKind};

lazy_static! {
  static ref WHITESPACE: Regex = Regex::from_str(r"\s").unwrap();
  static ref IDENT_START: Regex = Regex::from_str(r"[A-Za-z_]").unwrap();
  static ref IDENT: Regex = Regex::from_str(r"[A-Za-z0-9_.]").unwrap();
}

#[derive(Debug)]
pub struct Lexer<'a> {
  src: StringReader<'a>,
  tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
  pub fn new(src: &'a str) -> Self {
    Self {
      src: StringReader::new(src),
      tokens: vec![],
    }
  }

  fn read_number(&mut self) -> Token {
    let (line, pos) = self.src.position();
    let mut dot_met = false;
    let mut literal = String::new();

    loop {
      let sym = self.src.peek();
      if let Some(sym) = sym {
        if sym.is_ascii_digit() {
          literal.push(sym);
        } else if sym == '.' {
          if !dot_met {
            dot_met = true;
            literal.push(sym);
          } else {
            break;
          }
        } else {
          break;
        }
      } else {
        break;
      }
      self.src.advance();
    }

    Token {
      src: literal,
      kind: if dot_met {
        TokenKind::Float
      } else {
        TokenKind::Integer
      },
      line,
      pos,
    }
  }

  pub fn read_identifier(&mut self) -> Token {
    let (line, pos) = self.src.position();
    let mut literal = String::new();
    loop {
      let sym = self.src.peek();
      if let Some(sym) = sym {
        let s = String::from(sym);
        if IDENT.is_match(&s) {
          literal.push(sym);
        } else {
          break;
        }
      } else {
        break;
      }
      self.src.advance();
    }

    let lower = literal.to_lowercase();
    match lower.as_str() {
      "and" => Token {
        src: literal,
        kind: TokenKind::And,
        line,
        pos,
      },
      "or" => Token {
        src: literal,
        kind: TokenKind::Or,
        line,
        pos,
      },
      _ => Token {
        src: literal,
        kind: TokenKind::Ident,
        line,
        pos,
      },
    }
  }

  fn read_equals_or_matches(&mut self) -> Token {
    let (line, pos) = self.src.position();
    self.src.advance();
    let sym = self.src.peek();
    if let Some(sym) = sym {
      if sym == '=' {
        self.src.advance();
        Token {
          src: "==".into(),
          kind: TokenKind::Equals,
          line,
          pos,
        }
      } else if sym == '~' {
        self.src.advance();
        Token {
          src: "=~".into(),
          kind: TokenKind::Matches,
          line,
          pos,
        }
      } else {
        Token {
          src: "=".into(),
          kind: TokenKind::Equals,
          line,
          pos,
        }
      }
    } else {
      Token {
        src: "=".into(),
        kind: TokenKind::Equals,
        line,
        pos,
      }
    }
  }

  fn read_not_equals_or_not_matches(&mut self) -> Token {
    let (line, pos) = self.src.position();
    self.src.advance();
    let sym = self.src.peek();
    if let Some(sym) = sym {
      match sym {
        '=' => {
          self.src.advance();
          Token {
            src: "!=".into(),
            kind: TokenKind::NotEquals,
            line,
            pos,
          }
        }
        '~' => {
          self.src.advance();
          Token {
            src: "!~".into(),
            kind: TokenKind::NotMatches,
            line,
            pos,
          }
        }
        _ => Token {
          src: "!".into(),
          kind: TokenKind::Illegal,
          line,
          pos,
        },
      }
    } else {
      Token {
        src: "!".into(),
        kind: TokenKind::Illegal,
        line,
        pos,
      }
    }
  }

  fn read_less(&mut self) -> Token {
    let (line, pos) = self.src.position();
    self.src.advance();
    let sym = self.src.peek();
    if let Some(sym) = sym {
      match sym {
        '=' => {
          self.src.advance();
          Token {
            src: "<=".into(),
            kind: TokenKind::LessOrEqual,
            line,
            pos,
          }
        }
        _ => Token {
          src: "<".into(),
          kind: TokenKind::Less,
          line,
          pos,
        },
      }
    } else {
      Token {
        src: "<".into(),
        kind: TokenKind::Less,
        line,
        pos,
      }
    }
  }

  fn read_greater(&mut self) -> Token {
    let (line, pos) = self.src.position();
    self.src.advance();
    let sym = self.src.peek();
    if let Some(sym) = sym {
      match sym {
        '=' => {
          self.src.advance();
          Token {
            src: ">=".into(),
            kind: TokenKind::GreaterOrEqual,
            line,
            pos,
          }
        }
        _ => Token {
          src: ">".into(),
          kind: TokenKind::Greater,
          line,
          pos,
        },
      }
    } else {
      Token {
        src: ">".into(),
        kind: TokenKind::Greater,
        line,
        pos,
      }
    }
  }

  fn read_string(&mut self) -> Token {
    let (line, pos) = self.src.position();
    let mut literal = String::new();
    let mut escape = false;
    self.src.advance();

    loop {
      let sym = self.src.peek();
      self.src.advance();
      if let Some(sym) = sym {
        match sym {
          '\n' | '\t' | '\r' => {
            return Token {
              src: literal,
              kind: TokenKind::Illegal,
              line,
              pos,
            }
          }
          _ => {
            if escape {
              match sym {
                'n' => literal.push('\n'),
                't' => literal.push('\t'),
                'r' => literal.push('\r'),
                _ => literal.push(sym),
              }
              escape = false
            } else {
              match sym {
                '\\' => escape = true,
                '"' => break,
                _ => literal.push(sym),
              }
            }
          }
        }
      } else {
        return Token {
          src: literal,
          kind: TokenKind::Illegal,
          line,
          pos,
        };
      }
    }
    Token {
      src: literal,
      kind: TokenKind::String,
      line,
      pos,
    }
  }

  pub fn parse(&mut self) -> TokenFlow {
    loop {
      let sym = self.src.peek();
      if let Some(sym) = sym {
        let s = String::from(sym);
        let token = if sym.is_ascii_digit() {
          self.read_number()
        } else if IDENT_START.is_match(&s) {
          self.read_identifier()
        } else if sym == '=' {
          self.read_equals_or_matches()
        } else if sym == '!' {
          self.read_not_equals_or_not_matches()
        } else if sym == '<' {
          self.read_less()
        } else if sym == '>' {
          self.read_greater()
        } else if sym == '"' {
          self.read_string()
        } else if sym == '(' {
          let (line, pos) = self.src.position();
          self.src.advance();
          Token {
            src: "(".into(),
            kind: TokenKind::LeftBrace,
            line,
            pos,
          }
        } else if sym == ')' {
          let (line, pos) = self.src.position();
          self.src.advance();
          Token {
            src: ")".into(),
            kind: TokenKind::RightBrace,
            line,
            pos,
          }
        } else if WHITESPACE.is_match(&s) {
          self.src.advance();
          continue;
        } else {
          let (line, pos) = self.src.position();
          self.src.advance();
          Token {
            src: String::from(sym),
            kind: TokenKind::Illegal,
            line,
            pos,
          }
        };

        let illegal = token.kind == TokenKind::Illegal;
        self.tokens.push(token);
        if illegal {
          break;
        }
      } else {
        break;
      }
    }

    if self.src.peek().is_none() {
      let (line, pos) = self.src.position();
      self.tokens.push(Token {
        src: String::new(),
        kind: TokenKind::EOF,
        line,
        pos,
      })
    }

    TokenFlow::new(&self.tokens)
  }
}
