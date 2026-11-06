//! Typed errors for rule parsing.

/// A permission rule the policy could not use. [`crate::Policy::new`] skips
/// such rules and returns these errors so the caller can surface them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("invalid permission rule `{rule}`: {message}")]
    InvalidRule { rule: String, message: String },
    #[error("invalid path pattern in permission rule `{rule}`: {source}")]
    InvalidPattern {
        rule: String,
        #[source]
        source: globset::Error,
    },
}

impl PolicyError {
    pub(crate) fn invalid(rule: &str, message: impl Into<String>) -> Self {
        Self::InvalidRule {
            rule: rule.to_string(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn messages_name_the_rule() {
        let error = PolicyError::invalid("Bash(", "missing closing parenthesis");
        assert_eq!(
            error.to_string(),
            "invalid permission rule `Bash(`: missing closing parenthesis"
        );
        assert!(error.source().is_none());
    }

    #[test]
    fn pattern_errors_keep_the_glob_cause() {
        let source = globset::Glob::new("src/[").unwrap_err();
        let error = PolicyError::InvalidPattern {
            rule: "Read(src/[)".into(),
            source,
        };
        assert!(error.to_string().starts_with("invalid path pattern"));
        assert!(error.source().is_some());
    }
}
