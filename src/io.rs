//! Typed outcomes of the product's own local control transport.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Connect,
    Request,
    Header,
    Payload,
}

impl std::fmt::Display for Phase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Connect => "connect",
            Self::Request => "request",
            Self::Header => "reply header",
            Self::Payload => "reply payload",
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IoError {
    #[error("browser operation configuration is invalid")]
    InvalidConfiguration,
    #[error("browser client capacity is exhausted")]
    Busy,
    #[error("browser host is unavailable")]
    Unavailable,
    #[error("browser control operation timed out during {0}")]
    Timeout(Phase),
    #[error("browser operation was cancelled")]
    Cancelled,
    #[error("browser response exceeds its byte limit")]
    OutputLimit,
    #[error("browser response framing is invalid")]
    InvalidFrame,
    #[error("browser host response is invalid")]
    InvalidResponse,
    #[error("browser control transport failed")]
    Transport,
}

impl IoError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "invalid_configuration",
            Self::Busy => "busy",
            Self::Unavailable => "unavailable",
            Self::Timeout(_) => "timeout",
            Self::Cancelled => "cancelled",
            Self::OutputLimit => "output_limit",
            Self::InvalidFrame => "invalid_frame",
            Self::InvalidResponse => "invalid_response",
            Self::Transport => "transport",
        }
    }
}

pub(crate) fn transport(error: std::io::Error, phase: Phase) -> IoError {
    match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            IoError::Unavailable
        }
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => IoError::Timeout(phase),
        std::io::ErrorKind::UnexpectedEof => IoError::InvalidFrame,
        _ => IoError::Transport,
    }
}
