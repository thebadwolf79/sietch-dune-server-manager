//! Validation helpers for values passed to shells, PowerShell, and Kubernetes.

use crate::errors::failure;
use crate::models::CommandResult;

/// Validates a Kubernetes identifier used as a command argument.
pub fn validate_kube_arg(value: &str, label: &str) -> CommandResult<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err(failure(format!("Invalid Kubernetes {label}: {value}")));
    }
    Ok(())
}

/// Validates a required single-line plain-text value.
pub fn validate_plain_value(value: &str, label: &str) -> CommandResult<()> {
    if value.is_empty() || value.chars().any(|ch| ch.is_control()) {
        return Err(failure(format!("{label} is not configured")));
    }
    Ok(())
}

/// Resolves an optional value against a fallback and validates the result.
pub fn required_config_value(
    value: Option<String>,
    fallback: &str,
    label: &str,
) -> CommandResult<String> {
    let value = value
        .unwrap_or_else(|| fallback.to_string())
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(failure(format!("{label} is not configured")));
    }
    Ok(value)
}
