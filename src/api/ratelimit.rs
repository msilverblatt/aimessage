use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;

pub type SharedLimiter = Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>;

pub fn create_limiter(requests_per_minute: u32) -> SharedLimiter {
    Arc::new(RateLimiter::direct(
        Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap()),
    ))
}

pub async fn rate_limit(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let limiter = request.extensions().get::<SharedLimiter>().cloned();
    if let Some(limiter) = limiter {
        if limiter.check().is_err() {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    Ok(next.run(request).await)
}
