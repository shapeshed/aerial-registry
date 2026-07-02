use rusqlite::{Connection, params};
use tracing::{info, warn};

use crate::station::Station;

const DEFAULT_PATH: &str = "state.db";
const STALE_AFTER_DAYS: u32 = 90;

/// Persistent station state carried between nightly runs. Records when each
/// station was first and last discovered and how many consecutive liveness
/// checks it has failed, so a single bad night does not prune a station.
///
/// Keyed on `(provider, provider_id)`, falling back to the stream URL for
/// providers that emit no id. The database lives at `state.db` (override with
/// `AERIAL_STATE_DB`; set it to an empty string to disable, which also
/// disables liveness hysteresis).
pub struct StateStore {
    conn: Connection,
}

/// The identity a station keeps across runs.
pub struct StationKey {
    provider: String,
    id: String,
}

impl StationKey {
    pub fn of(station: &Station) -> Self {
        let id = match station.provider_id.as_deref() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => station.stream_url.clone(),
        };
        Self {
            provider: station.provider.clone(),
            id,
        }
    }
}

pub fn open_from_env() -> Option<StateStore> {
    let path = match std::env::var("AERIAL_STATE_DB") {
        Ok(v) if v.is_empty() => {
            info!("Station state store disabled");
            return None;
        }
        Ok(v) => v,
        Err(_) => DEFAULT_PATH.to_string(),
    };
    match StateStore::open(&path) {
        Ok(store) => Some(store),
        Err(e) => {
            warn!(path, error = %e, "Could not open state store; liveness hysteresis disabled");
            None
        }
    }
}

impl StateStore {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.execute_batch(
            "create table if not exists station_state(
                provider text not null,
                provider_id text not null,
                first_seen text not null,
                last_seen text not null,
                consecutive_failures integer not null default 0,
                last_status text,
                source_hash text,
                primary key(provider, provider_id)
            );",
        )?;
        let stale = conn.execute(
            "delete from station_state
             where last_seen < date('now', ?1)",
            params![format!("-{STALE_AFTER_DAYS} days")],
        )?;
        if stale > 0 {
            info!(rows = stale, "Removed stale station state");
        }
        Ok(Self { conn })
    }

    /// Upsert discovery timestamps for every station in this run.
    pub fn record_seen(&self, keys: &[StationKey]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "insert into station_state(provider, provider_id, first_seen, last_seen)
                 values(?1, ?2, date('now'), date('now'))
                 on conflict(provider, provider_id) do update set last_seen = date('now')",
            )?;
            for key in keys {
                stmt.execute(params![key.provider, key.id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Reset the failure streak for stations that passed liveness.
    pub fn record_live(&self, keys: &[StationKey]) -> anyhow::Result<()> {
        self.set_status(keys, "live", true)
    }

    /// Geo-suspect responses are recorded but never counted as failures: a
    /// 403/451 from the build machine says nothing about the listener.
    pub fn record_geo_blocked(&self, keys: &[StationKey]) -> anyhow::Result<()> {
        self.set_status(keys, "geo_blocked", false)
    }

    /// Increment failure streaks and return the new count per key, in input
    /// order.
    pub fn record_failures(&self, items: &[(StationKey, &str)]) -> anyhow::Result<Vec<u32>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut counts = Vec::with_capacity(items.len());
        {
            let mut update = tx.prepare(
                "update station_state
                 set consecutive_failures = consecutive_failures + 1, last_status = ?3
                 where provider = ?1 and provider_id = ?2",
            )?;
            let mut read = tx.prepare(
                "select consecutive_failures from station_state
                 where provider = ?1 and provider_id = ?2",
            )?;
            for (key, status) in items {
                update.execute(params![key.provider, key.id, status])?;
                let count: u32 = read.query_row(params![key.provider, key.id], |row| row.get(0))?;
                counts.push(count);
            }
        }
        tx.commit()?;
        Ok(counts)
    }

    fn set_status(&self, keys: &[StationKey], status: &str, reset: bool) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let sql = if reset {
                "update station_state
                 set consecutive_failures = 0, last_status = ?3
                 where provider = ?1 and provider_id = ?2"
            } else {
                "update station_state set last_status = ?3
                 where provider = ?1 and provider_id = ?2"
            };
            let mut stmt = tx.prepare(sql)?;
            for key in keys {
                stmt.execute(params![key.provider, key.id, status])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    fn row(&self, key: &StationKey) -> (String, String, u32, Option<String>) {
        self.conn
            .query_row(
                "select first_seen, last_seen, consecutive_failures, last_status
                 from station_state where provider = ?1 and provider_id = ?2",
                params![key.provider, key.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(provider: &str, id: &str) -> StationKey {
        StationKey {
            provider: provider.to_string(),
            id: id.to_string(),
        }
    }

    #[test]
    fn key_falls_back_to_stream_url() {
        let station = Station {
            name: "X".to_string(),
            stream_url: "https://example.com/x".to_string(),
            logo_url: None,
            country: None,
            country_code: None,
            tags: vec![],
            description: None,
            provider: "curated".to_string(),
            provider_id: None,
            trusted: false,
        };
        let k = StationKey::of(&station);
        assert_eq!(k.id, "https://example.com/x");
    }

    #[test]
    fn failures_accumulate_and_reset_on_success() {
        let store = StateStore::open_in_memory().unwrap();
        let k = || vec![key("bbc", "one")];
        store.record_seen(&k()).unwrap();

        let counts = store
            .record_failures(&[(key("bbc", "one"), "unreachable")])
            .unwrap();
        assert_eq!(counts, vec![1]);
        let counts = store
            .record_failures(&[(key("bbc", "one"), "unreachable")])
            .unwrap();
        assert_eq!(counts, vec![2]);

        store.record_live(&k()).unwrap();
        let (_, _, failures, status) = store.row(&key("bbc", "one"));
        assert_eq!(failures, 0);
        assert_eq!(status.as_deref(), Some("live"));
    }

    #[test]
    fn geo_blocked_does_not_touch_failure_streak() {
        let store = StateStore::open_in_memory().unwrap();
        store.record_seen(&[key("bbc", "one")]).unwrap();
        store
            .record_failures(&[(key("bbc", "one"), "unreachable")])
            .unwrap();
        store.record_geo_blocked(&[key("bbc", "one")]).unwrap();
        let (_, _, failures, status) = store.row(&key("bbc", "one"));
        assert_eq!(failures, 1);
        assert_eq!(status.as_deref(), Some("geo_blocked"));
    }

    #[test]
    fn record_seen_preserves_first_seen() {
        let store = StateStore::open_in_memory().unwrap();
        store
            .conn
            .execute(
                "insert into station_state(provider, provider_id, first_seen, last_seen)
                 values('bbc', 'one', '2020-01-01', '2020-01-01')",
                [],
            )
            .unwrap();
        store.record_seen(&[key("bbc", "one")]).unwrap();
        let (first, last, _, _) = store.row(&key("bbc", "one"));
        assert_eq!(first, "2020-01-01");
        assert_ne!(last, "2020-01-01");
    }
}
