use super::{condition::Condition, error::CompileError};

#[derive(Debug)]
pub enum CombineOperator {
  And,
  Or,
}

pub type EvaluateFunc<T> = dyn Fn(&T) -> bool + Send + Sync;
pub type CompileFunc<T> = dyn Fn(Condition) -> Result<Box<EvaluateFunc<T>>, CompileError>;

pub enum LeftExpression<T> {
  Expression(Expression<T>),
  Condition(Condition),
  CompiledFilter(Box<EvaluateFunc<T>>),
}

pub struct Expression<T> {
  pub left: Box<LeftExpression<T>>,
  pub operator: Option<CombineOperator>,
  pub right: Option<Box<Expression<T>>>,
}

impl<T> Expression<T> {
  pub fn compile(&mut self, cb: &CompileFunc<T>) -> Result<(), CompileError> {
    match self.left.as_mut() {
      LeftExpression::Expression(expr) => {
        expr.compile(cb)?;
      }
      LeftExpression::Condition(cond) => {
        let compiled = cb(cond.clone())?;
        self.left = Box::new(LeftExpression::CompiledFilter(compiled));
      }
      _ => (), // TODO: already compiled error
    }

    if let Some(right) = self.right.as_mut() {
      right.compile(cb)?;
    }

    Ok(())
  }

  pub fn evaluate(&self, model: &T) -> bool {
    let left_result = match self.left.as_ref() {
      LeftExpression::CompiledFilter(filter) => filter(model),
      LeftExpression::Expression(e) => e.evaluate(model),
      _ => false, // TODO: partially compiled error
    };

    if self.operator.is_none() {
      left_result
    } else {
      let right = self.right.as_ref().unwrap();
      match self.operator.as_ref().unwrap() {
        CombineOperator::And => left_result && right.evaluate(model),
        CombineOperator::Or => left_result || right.evaluate(model),
      }
    }
  }
}
