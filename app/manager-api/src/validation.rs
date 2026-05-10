use crate::{errors::ApiError, state::AppState};

pub fn validate_kube_name(value: &str) -> Result<(), ApiError> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err(ApiError::bad_request("invalid Kubernetes resource name"));
    }
    Ok(())
}

pub fn validate_namespace(state: &AppState, namespace: &str) -> Result<(), ApiError> {
    validate_kube_name(namespace)?;
    if namespace != state.namespace {
        return Err(ApiError::bad_request(format!(
            "manager API is scoped to namespace {}",
            state.namespace
        )));
    }
    Ok(())
}
