use axum::{http::header::HeaderMap, http::header::AUTHORIZATION};

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers.get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }
    None
}
