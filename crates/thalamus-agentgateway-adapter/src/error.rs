use std::fmt;
use std::time::Duration;

/// Typed errors from the Agentgateway adapter.
#[derive(Debug)]
pub enum AdapterError {
    Connection { detail: String },
    Timeout { duration: Duration },
    ServerError { status: u16, body: String },
    MalformedResponse { reason: String },
    ModelMapping { model: String },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::Connection { detail } => {
                write!(f, "connection error: {detail}")
            }
            AdapterError::Timeout { duration } => {
                write!(f, "request timed out after {:?}", duration)
            }
            AdapterError::ServerError { status, body } => {
                write!(f, "server returned {status}: {body}")
            }
            AdapterError::MalformedResponse { reason } => {
                write!(f, "malformed response: {reason}")
            }
            AdapterError::ModelMapping { model } => {
                write!(f, "no model mapping for backend handle: {model}")
            }
        }
    }
}

impl std::error::Error for AdapterError {}
