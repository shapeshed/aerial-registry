use std::time::Duration;

use async_trait::async_trait;
use backon::{BackoffBuilder, ExponentialBuilder};
use http::Extensions;
use reqwest::{Request, Response, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Error, Middleware, Next};

const USER_AGENT: &str = concat!(
    "aerial-registry/",
    env!("CARGO_PKG_VERSION"),
    " (aerial-registry)"
);

/// The shared HTTP client, wrapped in retry middleware so **every** request
/// from **every** provider is retried on transient failures with exponential
/// backoff. See [`RetryMiddleware`].
pub type Client = ClientWithMiddleware;

pub fn build_client() -> reqwest::Result<Client> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(15))
        .build()?;
    Ok(ClientBuilder::new(client).with(RetryMiddleware).build())
}

/// Build a URL with percent-encoded query parameters. Needed because the
/// middleware request builder does not expose `query`.
pub fn url_with_query(base: &str, params: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse(base).expect("a valid absolute URL");
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in params {
            pairs.append_pair(key, value);
        }
    }
    url.to_string()
}

/// Retries transient failures — `5xx`, `429`, `408`, timeouts and connect
/// errors — with exponential backoff (250ms → 5s, jittered, up to three
/// retries). `4xx` responses and non-transient errors are returned immediately.
#[derive(Clone, Copy)]
struct RetryMiddleware;

#[async_trait]
impl Middleware for RetryMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        let mut backoff = ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(250))
            .with_max_delay(Duration::from_secs(5))
            .with_max_times(3)
            .with_jitter()
            .build();

        loop {
            // A request with a non-cloneable body cannot be replayed, so send it
            // once without retrying.
            let Some(attempt) = req.try_clone() else {
                return next.run(req, extensions).await;
            };

            let result = next.clone().run(attempt, extensions).await;
            let retry = match &result {
                Ok(resp) => is_transient_status(resp.status()),
                Err(e) => is_transient_error(e),
            };
            if !retry {
                return result;
            }

            match backoff.next() {
                Some(delay) => {
                    tracing::debug!(delay = ?delay, "Retrying request");
                    tokio::time::sleep(delay).await;
                }
                None => return result,
            }
        }
    }
}

fn is_transient_status(status: StatusCode) -> bool {
    status.is_server_error()
        || status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::REQUEST_TIMEOUT
}

fn is_transient_error(error: &Error) -> bool {
    match error {
        Error::Reqwest(e) => e.is_timeout() || e.is_connect() || e.is_request() || e.is_body(),
        Error::Middleware(_) => false,
    }
}
