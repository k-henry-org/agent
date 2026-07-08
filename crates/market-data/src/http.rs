//! Shared HTTP plumbing for live REST adapters (ROADMAP P7.3).
//!
//! One JSON client with sane timeouts and a **bounded** retry/backoff loop, mapping every
//! transport/status failure onto a typed [`ProviderError`]. Keeping the network concerns
//! here means an adapter like [`crate::MassiveSource`] only has to say *what* to fetch.
//!
//! **Rate-limit stance.** A `429` is surfaced **immediately** as
//! [`ProviderError::RateLimited`] carrying the parsed `Retry-After` hint — we do *not* block
//! the call for the server's requested wait (which can be minutes). The caller (a scan, a
//! future scheduler) decides when to try again, which is the honest place to honor it; a
//! quick blind re-hit would just be rate-limited again. Only transient failures (`5xx`,
//! transport errors) are retried, with short exponential backoff.

use std::time::Duration;

use exub_core::{ProviderError, ProviderResult};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;

/// Total attempts for one request: the initial try plus retries of transient failures.
const MAX_ATTEMPTS: u32 = 3;

/// A JSON HTTP client with typed errors and bounded retries.
#[derive(Debug)]
pub struct HttpClient {
    inner: reqwest::Client,
    /// Base backoff between transient-failure retries; production uses 500ms, tests shrink
    /// it to ~0 so the retry path is exercised without real waiting.
    backoff_base: Duration,
}

impl HttpClient {
    /// Build a client with connect/total timeouts. Fails only if the TLS backend won't init.
    ///
    /// # Errors
    /// [`ProviderError::Transport`] if the underlying client can't be constructed.
    pub fn new() -> ProviderResult<Self> {
        let inner = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ProviderError::Transport(format!("building HTTP client: {e}")))?;
        Ok(Self {
            inner,
            backoff_base: Duration::from_millis(500),
        })
    }

    /// Override the retry backoff base — tests set this to ~1ms so retries don't sleep.
    #[must_use]
    pub fn with_backoff_base(mut self, base: Duration) -> Self {
        self.backoff_base = base;
        self
    }

    /// GET `url` with a bearer `token`, decoding the JSON body into `T`.
    ///
    /// The token rides the `Authorization` header, never the URL, so it can't leak into a
    /// log line or an error message. Transient failures retry with backoff; a `429` returns
    /// [`ProviderError::RateLimited`] immediately with the `Retry-After` hint.
    ///
    /// # Errors
    /// `Auth` (401/403), `NotFound` (404), `RateLimited` (429), or `Transport` (5xx,
    /// timeouts, decode failures, exhausted retries).
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        token: &SecretString,
    ) -> ProviderResult<T> {
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self
                .inner
                .get(url)
                .bearer_auth(token.expose_secret())
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp.json::<T>().await.map_err(|e| {
                            ProviderError::Transport(format!("decoding JSON body: {e}"))
                        });
                    }
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        // Surface the hint, don't block on it — see the module note.
                        let retry_after = parse_retry_after(
                            resp.headers()
                                .get(reqwest::header::RETRY_AFTER)
                                .and_then(|v| v.to_str().ok()),
                        );
                        return Err(ProviderError::RateLimited { retry_after });
                    }
                    if status.is_server_error() && attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(backoff_delay(self.backoff_base, attempt)).await;
                        continue;
                    }
                    return Err(status_to_error(status));
                }
                Err(e) => {
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(backoff_delay(self.backoff_base, attempt)).await;
                        continue;
                    }
                    return Err(ProviderError::Transport(format!("request failed: {e}")));
                }
            }
        }
    }
}

/// Map a non-success, non-retried HTTP status onto a typed error.
fn status_to_error(status: reqwest::StatusCode) -> ProviderError {
    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            ProviderError::Auth(format!("HTTP {}", status.as_u16()))
        }
        reqwest::StatusCode::NOT_FOUND => {
            ProviderError::NotFound(format!("HTTP {}", status.as_u16()))
        }
        _ => ProviderError::Transport(format!("HTTP {}", status.as_u16())),
    }
}

/// Parse a `Retry-After` header. Only the delta-seconds form is understood; the HTTP-date
/// form (rare in practice for APIs) maps to `None` rather than a wrong guess.
fn parse_retry_after(raw: Option<&str>) -> Option<Duration> {
    raw?.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Exponential backoff for a 1-based attempt: `base * 2^(attempt-1)`, saturating.
fn backoff_delay(base: Duration, attempt: u32) -> Duration {
    base.saturating_mul(1u32 << (attempt - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_retry_after_reads_delta_seconds() {
        assert_eq!(parse_retry_after(Some("7")), Some(Duration::from_secs(7)));
        assert_eq!(
            parse_retry_after(Some("  2 ")),
            Some(Duration::from_secs(2))
        );
        // HTTP-date form and garbage are not guessed at.
        assert_eq!(
            parse_retry_after(Some("Wed, 21 Oct 2015 07:28:00 GMT")),
            None
        );
        assert_eq!(parse_retry_after(Some("")), None);
        assert_eq!(parse_retry_after(None), None);
    }

    #[test]
    fn backoff_doubles_per_attempt() {
        let base = Duration::from_millis(100);
        assert_eq!(backoff_delay(base, 1), Duration::from_millis(100));
        assert_eq!(backoff_delay(base, 2), Duration::from_millis(200));
        assert_eq!(backoff_delay(base, 3), Duration::from_millis(400));
    }

    #[test]
    fn status_mapping_is_typed() {
        assert!(matches!(
            status_to_error(reqwest::StatusCode::UNAUTHORIZED),
            ProviderError::Auth(_)
        ));
        assert!(matches!(
            status_to_error(reqwest::StatusCode::FORBIDDEN),
            ProviderError::Auth(_)
        ));
        assert!(matches!(
            status_to_error(reqwest::StatusCode::NOT_FOUND),
            ProviderError::NotFound(_)
        ));
        assert!(matches!(
            status_to_error(reqwest::StatusCode::BAD_GATEWAY),
            ProviderError::Transport(_)
        ));
    }
}
