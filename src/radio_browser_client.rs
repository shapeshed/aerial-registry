use std::future::Future;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tracing::warn;

const MAX_RETRIES: u32 = 3;
const RETRY_BASE_MS: u64 = 500;

#[derive(Deserialize)]
struct RbServer {
    name: String,
}

/// Discover the current list of Radio Browser mirror servers. The public
/// API is served from several independently-run mirrors; querying
/// `all.api.radio-browser.info` returns the current list so callers can
/// rotate across them instead of hammering one host that might be the one
/// currently returning 502s.
pub async fn discover_servers(client: &Client) -> anyhow::Result<Vec<String>> {
    let servers: Vec<RbServer> = client
        .get("https://all.api.radio-browser.info/json/servers")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("request failed: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("parse failed: {e}"))?;

    if servers.is_empty() {
        anyhow::bail!("server list was empty");
    }

    Ok(servers.into_iter().map(|s| s.name).collect())
}

/// Retry `op` against Radio Browser mirrors, rotating servers on each
/// attempt with exponential backoff (500ms, 1s, 2s). `op` receives the
/// mirror host to hit this attempt. Returns `None` once retries are
/// exhausted — callers decide what an exhausted lookup means for them
/// (skip a station's tags, drop a page, or return no stations at all).
pub async fn with_retry<T, F, Fut>(servers: &[String], context: &str, mut op: F) -> Option<T>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    for attempt in 0..MAX_RETRIES {
        // Owned, not borrowed: keeps the per-call future's lifetime
        // independent of `servers`, which a generic FnMut -> Future bound
        // can't otherwise express without higher-ranked trait bounds.
        let server = servers[attempt as usize % servers.len()].clone();
        match op(server.clone()).await {
            Ok(value) => return Some(value),
            Err(e) => {
                let delay_ms = RETRY_BASE_MS * 2u64.pow(attempt);
                warn!(
                    context,
                    server,
                    attempt = attempt + 1,
                    delay_ms,
                    error = %e,
                    "Radio Browser request failed, retrying"
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
    warn!(context, "Radio Browser request exhausted retries");
    None
}

#[cfg(test)]
mod tests {
    use super::with_retry;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn returns_value_on_first_success() {
        let calls = AtomicUsize::new(0);
        let servers = vec!["a".to_string(), "b".to_string()];
        let result = with_retry(&servers, "ctx", |server| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move { Ok::<_, anyhow::Error>(server) }
        })
        .await;
        assert_eq!(result, Some("a".to_string()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn rotates_servers_and_recovers_after_failures() {
        let servers = vec!["a".to_string(), "b".to_string()];
        let seen = std::sync::Mutex::new(Vec::new());
        let result = with_retry(&servers, "ctx", |server| {
            seen.lock().unwrap().push(server.clone());
            async move {
                if server == "a" {
                    Err(anyhow::anyhow!("a is down"))
                } else {
                    Ok(server)
                }
            }
        })
        .await;
        assert_eq!(result, Some("b".to_string()));
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_max_retries() {
        let servers = vec!["a".to_string()];
        let result = with_retry(&servers, "ctx", |_server| async move {
            Err::<(), _>(anyhow::anyhow!("always fails"))
        })
        .await;
        assert_eq!(result, None);
    }
}
