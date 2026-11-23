use std::fmt;

/// Stable application error classes. The contained text is the human CLI
/// message and is deliberately rendered without decoration by `Display`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Validation {
        field: &'static str,
        message: String,
    },
    NotFound {
        what: &'static str,
        id: String,
    },
    Ambiguous {
        id: String,
    },
    Conflict(String),
    Busy(String),
    Storage(String),
    SourceUnavailable(String),
    Config(String),
    Internal(String),
}

impl Error {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "validation",
            Self::NotFound { .. } => "not_found",
            Self::Ambiguous { .. } => "ambiguous",
            Self::Conflict(_) => "conflict",
            Self::Busy(_) => "busy",
            Self::Storage(_) => "storage",
            Self::SourceUnavailable(_) => "source_unavailable",
            Self::Config(_) => "config",
            Self::Internal(_) => "internal",
        }
    }

    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            field,
            message: message.into(),
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Convenience retained for callers that previously inspected string errors.
    pub fn contains(&self, pattern: &str) -> bool {
        self.to_string().contains(pattern)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { message, .. }
            | Self::Conflict(message)
            | Self::Busy(message)
            | Self::Storage(message)
            | Self::SourceUnavailable(message)
            | Self::Config(message)
            | Self::Internal(message) => f.write_str(message),
            Self::NotFound { what, id } if *what == "item" && id.chars().count() < 4 => {
                write!(f, "no item {id:?}; use at least 4 characters of the id")
            }
            Self::NotFound { what, id } => write!(f, "no {what} {id:?}"),
            Self::Ambiguous { id } => {
                write!(f, "item id {id:?} is ambiguous; use more characters")
            }
        }
    }
}

impl std::error::Error for Error {}

// String-producing validators remain common during this migration. The
// fallback keeps their messages intact while assigning the public class.
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::validation("input", message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::validation("input", message)
    }
}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
}
