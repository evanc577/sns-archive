use reqwest::{Response, StatusCode};
use std::time::UNIX_EPOCH;
use tokio::time::Duration;

static RESET_HEADER: &str = "x-rate-limit-reset";

pub fn check_rate_limit(resp: &Response) -> Option<Duration> {
    if resp.status() != StatusCode::TOO_MANY_REQUESTS {
        return None;
    }

    let rate_reset_at = resp.headers().get(RESET_HEADER)?.to_str().ok()?;
    let duration =
        Duration::from_secs(rate_reset_at.parse::<u64>().ok()?) - UNIX_EPOCH.elapsed().ok()?;
    Some(duration)
}
