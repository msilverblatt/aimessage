use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

pub async fn require_api_key(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected_key = request
        .extensions()
        .get::<ApiKey>()
        .map(|k| k.0.clone());

    let provided_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    match (expected_key, provided_key) {
        (Some(expected), Some(provided)) if expected == provided => {
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Clone)]
pub struct ApiKey(pub String);
