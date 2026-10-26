use thiserror::Error;

#[derive(Error, Debug)]
pub enum CircuitError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Singular matrix encountered during circuit solve: {0}")]
    SingularMatrix(String),

    #[error("Floating or unconnected node: {0}")]
    FloatingNode(String),

    #[error("Invalid circuit element: {0}")]
    InvalidElement(String),

    #[error("Numerical error: {0}")]
    Numerical(#[from] tw_num::error::NumericalError),
}
