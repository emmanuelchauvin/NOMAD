use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("Operator failure: {0}")]
    OperatorError(String),
    #[error("Evaluation error: {0}")]
    EvaluationError(String),
    #[error("Budget exceeded before initial population could be evaluated")]
    PrematureBudgetExhaustion,
}
