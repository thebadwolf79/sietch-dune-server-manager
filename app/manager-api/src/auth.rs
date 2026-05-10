use axum::http::{header, HeaderMap};

use crate::{errors::ApiError, state::AppState};

pub fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), ApiError> {
    let Some(expected) = state.token.as_deref() else {
        return Ok(());
    };

    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or(query_token)
        .or_else(|| cookie_token(headers));

    match bearer {
        Some(actual) if constant_time_eq(actual.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => Err(ApiError::unauthorized()),
    }
}

fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value.split(';').find_map(|part| {
                let (key, token) = part.trim().split_once('=')?;
                (key == "dune_manager_token").then_some(token)
            })
        })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}
