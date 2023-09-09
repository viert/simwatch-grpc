use self::{
  condition::{Condition, Operator, Value},
  error::ParseError,
  expression::{CombineOperator, Expression, LeftExpression},
};
use super::lexer::token::{TokenFlow, TokenKind};

pub mod condition;
pub mod error;
pub mod expression;

fn parse_operator(tf: &mut TokenFlow) -> Result<Operator, ParseError> {
  let token = tf.current().ok_or_else(|| {
    ParseError::UnexpectedEOS(vec![
      TokenKind::Equals,
      TokenKind::NotEquals,
      TokenKind::Matches,
      TokenKind::NotMatches,
      TokenKind::Less,
      TokenKind::Greater,
      TokenKind::LessOrEqual,
      TokenKind::GreaterOrEqual,
    ])
  })?;

  let operator = match token.kind {
    TokenKind::Equals => Operator::Equals,
    TokenKind::NotEquals => Operator::NotEquals,
    TokenKind::Matches => Operator::Matches,
    TokenKind::NotMatches => Operator::NotMatches,
    TokenKind::Less => Operator::Less,
    TokenKind::Greater => Operator::Greater,
    TokenKind::LessOrEqual => Operator::LessOrEqual,
    TokenKind::GreaterOrEqual => Operator::GreaterOrEqual,
    _ => {
      return Err(ParseError::UnexpectedTokenType(
        token.clone(),
        vec![
          TokenKind::Equals,
          TokenKind::NotEquals,
          TokenKind::Matches,
          TokenKind::NotMatches,
          TokenKind::Less,
          TokenKind::Greater,
          TokenKind::LessOrEqual,
          TokenKind::GreaterOrEqual,
        ],
      ))
    }
  };
  tf.advance();
  Ok(operator)
}

fn parse_value(tf: &mut TokenFlow) -> Result<Value, ParseError> {
  let token = tf.current().ok_or_else(|| {
    ParseError::UnexpectedEOS(vec![
      TokenKind::Integer,
      TokenKind::Float,
      TokenKind::String,
    ])
  })?;

  let value = match token.kind {
    TokenKind::Integer => {
      let val = token
        .src
        .parse::<i64>()
        .map_err(|err| ParseError::ConvertError(token.clone(), Box::new(err)))?;
      Value::Integer(val)
    }
    TokenKind::Float => {
      let val = token
        .src
        .parse::<f64>()
        .map_err(|err| ParseError::ConvertError(token.clone(), Box::new(err)))?;
      Value::Float(val)
    }
    TokenKind::String => Value::String(token.src.clone()),
    _ => {
      return Err(ParseError::UnexpectedEOS(vec![
        TokenKind::Integer,
        TokenKind::Float,
        TokenKind::String,
      ]))
    }
  };
  tf.advance();
  Ok(value)
}

fn parse_condition(tf: &mut TokenFlow) -> Result<Condition, ParseError> {
  let token = tf
    .current()
    .ok_or_else(|| ParseError::UnexpectedEOS(vec![TokenKind::Ident]))?;
  let ident = match token.kind {
    TokenKind::Ident => token.src.clone(),
    _ => {
      return Err(ParseError::UnexpectedTokenType(
        token.clone(),
        vec![TokenKind::Ident],
      ));
    }
  };
  tf.advance();

  let op_t = tf.current();
  let operator = parse_operator(tf)?;
  let value = parse_value(tf)?;

  let op_t = op_t.unwrap();

  match operator {
    Operator::Matches => match value {
      Value::Integer(_) => return Err(ParseError::InvalidValueType(op_t.clone(), vec!["string"])),
      Value::Float(_) => return Err(ParseError::InvalidValueType(op_t.clone(), vec!["string"])),
      Value::String(_) => (),
    },
    Operator::NotMatches => match value {
      Value::Integer(_) => return Err(ParseError::InvalidValueType(op_t.clone(), vec!["string"])),
      Value::Float(_) => return Err(ParseError::InvalidValueType(op_t.clone(), vec!["string"])),
      Value::String(_) => (),
    },
    Operator::Equals => (),
    Operator::NotEquals => (),
    _ => match value {
      Value::Integer(_) => (),
      Value::Float(_) => (),
      Value::String(_) => {
        return Err(ParseError::InvalidValueType(
          op_t.clone(),
          vec!["int", "float"],
        ))
      }
    },
  };

  Ok(Condition {
    ident,
    operator,
    value,
  })
}

fn parse_expression<T>(tf: &mut TokenFlow) -> Result<Expression<T>, ParseError> {
  let token = tf.current();
  if let Some(token) = token {
    let left = match token.kind {
      TokenKind::LeftBrace => {
        tf.advance();
        let exp = parse_expression(tf)?;
        let token = tf
          .current()
          .ok_or_else(|| ParseError::UnexpectedEOS(vec![TokenKind::RightBrace]))?;
        if token.kind == TokenKind::RightBrace {
          tf.advance();
          LeftExpression::Expression(exp)
        } else {
          return Err(ParseError::UnexpectedTokenType(
            token.clone(),
            vec![TokenKind::RightBrace],
          ));
        }
      }
      TokenKind::Ident => {
        let cond = parse_condition(tf)?;
        LeftExpression::Condition(cond)
      }
      _ => {
        return Err(ParseError::UnexpectedTokenType(
          token.clone(),
          vec![TokenKind::Ident, TokenKind::LeftBrace],
        ));
      }
    };
    let operator = tf
      .current()
      .filter(|token| matches!(token.kind, TokenKind::And | TokenKind::Or))
      .map(|token| match token.kind {
        TokenKind::And => CombineOperator::And,
        TokenKind::Or => CombineOperator::Or,
        _ => unreachable!(),
      });

    if operator.is_none() {
      Ok(Expression {
        left: Box::new(left),
        operator: None,
        right: None,
      })
    } else {
      tf.advance();
      let right = parse_expression(tf)?;
      Ok(Expression {
        left: Box::new(left),
        operator,
        right: Some(Box::new(right)),
      })
    }
  } else {
    Err(ParseError::UnexpectedEOS(vec![
      TokenKind::Ident,
      TokenKind::LeftBrace,
    ]))
  }
}

pub fn parse<T>(tf: &mut TokenFlow) -> Result<Expression<T>, ParseError> {
  let exp = parse_expression(tf)?;
  let token = tf.current();
  if let Some(token) = token {
    if token.kind == TokenKind::EOF {
      Ok(exp)
    } else {
      Err(ParseError::UnexpectedTokenType(
        token.clone(),
        vec![TokenKind::EOF],
      ))
    }
  } else {
    Err(ParseError::UnexpectedEOS(vec![TokenKind::EOF]))
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::lee::lexer::Lexer;
  use crate::lee::parser::error::CompileError;
  use crate::lee::parser::expression::{CompileFunc, EvaluateFunc};

  struct Model {
    x: i64,
    y: i64,
    callsign: String,
  }

  #[test]
  fn test_condition() {
    let mut l = Lexer::new("x > 5 AND y <= 7 && callsign =~ \"^AER\"");
    let mut tf = l.parse();
    let exp = parse_expression::<Model>(&mut tf);

    assert!(exp.is_ok());
    let mut exp = exp.unwrap();
    let cb: Box<CompileFunc<Model>> = Box::new(|cond| {
      let evalfunc: Box<EvaluateFunc<Model>> = match cond.ident.as_str() {
        "x" => Box::new(move |model| cond.value.eval_i64(model.x, cond.operator.clone())),
        "y" => Box::new(move |model| cond.value.eval_i64(model.y, cond.operator.clone())),
        "callsign" => {
          Box::new(move |model| cond.value.eval_str(&model.callsign, cond.operator.clone()))
        }
        _ => {
          return Err(CompileError {
            msg: "failed to compile, invalid identifier met".into(),
          })
        }
      };
      Ok(evalfunc)
    });
    let res = exp.compile(&cb);
    assert!(res.is_ok());

    let res = exp.evaluate(&Model {
      x: 9,
      y: 5,
      callsign: "AER384".into(),
    });
    assert!(res);

    let res = exp.evaluate(&Model {
      x: 3,
      y: 5,
      callsign: "AER391".into(),
    });
    assert!(!res);
  }
}
