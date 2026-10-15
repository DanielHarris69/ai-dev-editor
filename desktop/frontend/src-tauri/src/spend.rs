use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Datelike;

pub const PROVIDERS: &[&str] = &["hermes"];

#[derive(Clone)]
pub struct SpendState {
    pub db: Arc<Mutex<Connection>>,
    pub info: Arc<Mutex<ProxyInfo>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInfo {
    pub url: String,
    pub token: String,
    pub status: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageEntry {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpendRow {
    pub provider: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    pub requests: i64,
    pub session_count: i64,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub reasoning_efforts: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SeriesPoint {
    pub day: String,
    pub cost: f64,
    pub requests: i64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelSeries {
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub total_cost: f64,
    pub total_requests: i64,
    pub points: Vec<SeriesPoint>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpendSeries {
    pub window: String,
    pub from: String,
    pub to: String,
    pub total_cost: f64,
    pub total_requests: i64,
    pub days: Vec<String>,
    pub series: Vec<ModelSeries>,
    pub tokens_by_day: Vec<i64>,
    pub cost_by_day: Vec<f64>,
}

fn validate_optional_text(field: &str, value: Option<&str>, max: usize) -> Result<(), String> {
    if let Some(v) = value {
        if v.len() > max {
            return Err(format!("{field} too long"));
        }
    }
    Ok(())
}

fn migrate_legacy(db: &mut Connection, path: &std::path::Path) -> Result<(), String> {
    let columns: Vec<String> = db.prepare("PRAGMA table_info(api_spend)")
        .map_err(|e| e.to_string())?.query_map([], |r| r.get(1))
        .map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    if !columns.iter().any(|c| c == "source") {
        return Ok(());
    }
    let backup = path.with_extension(format!("pre-providers-{}.sqlite3", uuid::Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", params![backup.to_string_lossy().as_ref()])
        .map_err(|e| format!("Cannot back up legacy spend ledger: {e}"))?;
    let optional: Vec<_> = ["session_id", "reasoning_effort", "agent"].iter()
        .map(|name| if columns.iter().any(|c| c == name) { *name } else { "NULL" }).collect();
    let tx = db.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch("CREATE TABLE api_spend_upgrade (
        id INTEGER PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, provider TEXT NOT NULL,
        model TEXT NOT NULL, input_tokens INTEGER NOT NULL CHECK(input_tokens>=0),
        output_tokens INTEGER NOT NULL CHECK(output_tokens>=0), cost REAL NOT NULL CHECK(cost>=0),
        timestamp TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        session_id TEXT, reasoning_effort TEXT, agent TEXT);")
        .map_err(|e| e.to_string())?;
    tx.execute_batch(&format!("INSERT INTO api_spend_upgrade
        SELECT id,request_id,provider,model,input_tokens,output_tokens,cost,timestamp,{}
        FROM api_spend WHERE source='reported';
        DROP TABLE api_spend; ALTER TABLE api_spend_upgrade RENAME TO api_spend;", optional.join(",")))
        .map_err(|e| format!("Legacy ledger unchanged; migration failed: {e}"))?;
    tx.commit().map_err(|e| e.to_string())
}

impl SpendState {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut db = Connection::open(&path).map_err(|e| e.to_string())?;
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        migrate_legacy(&mut db, &path)?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
          CREATE TABLE IF NOT EXISTS api_spend (
            id INTEGER PRIMARY KEY, request_id TEXT NOT NULL, provider TEXT NOT NULL,
            model TEXT NOT NULL, input_tokens INTEGER NOT NULL CHECK(input_tokens>=0),
            output_tokens INTEGER NOT NULL CHECK(output_tokens>=0), cost REAL NOT NULL CHECK(cost>=0),
            timestamp TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            session_id TEXT, reasoning_effort TEXT, agent TEXT,
            UNIQUE(request_id));
          CREATE INDEX IF NOT EXISTS api_spend_ts ON api_spend(timestamp);
          CREATE INDEX IF NOT EXISTS api_spend_model ON api_spend(model, timestamp);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            info: Arc::new(Mutex::new(ProxyInfo {
                url: "http://127.0.0.1:4000".into(),
                token: uuid::Uuid::new_v4().to_string(),
                status: "starting".into(),
            })),
        })
    }

    #[cfg(test)]
    pub fn record(&self, entry: &UsageEntry) -> Result<bool, String> {
        self.record_allowing(entry, &[])
    }

    /// `extra_kinds` are provider kinds the user has configured; `hermes` is
    /// always accepted. A kind outside both is rejected so a stray integration
    /// cannot write spend under an unconfigured name.
    pub fn record_allowing(&self, entry: &UsageEntry, extra_kinds: &[String]) -> Result<bool, String> {
        let known = PROVIDERS.contains(&entry.provider.as_str())
            || extra_kinds.iter().any(|k| k == &entry.provider);
        if !known {
            return Err("Unsupported provider".into());
        }
        validate_optional_text("session_id", entry.session_id.as_deref(), 200)?;
        validate_optional_text("reasoning_effort", entry.reasoning_effort.as_deref(), 32)?;
        validate_optional_text("agent", entry.agent.as_deref(), 64)?;
        if let Some(r) = entry.reasoning_effort.as_deref() {
            if !matches!(r, "low" | "medium" | "high" | "minimal" | "none") {
                return Err("reasoning_effort must be low/medium/high/minimal/none".into());
            }
        }
        if entry.request_id.trim().is_empty()
            || entry.request_id.len() > 200
            || entry.model.trim().is_empty()
            || entry.model.len() > 200
            || entry.input_tokens < 0
            || entry.output_tokens < 0
            || entry.input_tokens > 1_000_000_000
            || entry.output_tokens > 1_000_000_000
            || !entry.cost.is_finite()
            || entry.cost < 0.0
            || entry.cost > 1_000_000.0
        {
            return Err("Invalid usage entry".into());
        }
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let existing = db.query_row(
            "SELECT provider,model,input_tokens,output_tokens,cost,session_id,reasoning_effort,agent FROM api_spend WHERE request_id=?1",
            params![entry.request_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            },
        );
        match existing {
            Ok(v)
                if v == (
                    entry.provider.clone(),
                    entry.model.clone(),
                    entry.input_tokens,
                    entry.output_tokens,
                    entry.cost,
                    entry.session_id.clone(),
                    entry.reasoning_effort.clone(),
                    entry.agent.clone(),
                ) =>
            {
                return Ok(false)
            }
            Ok(_) => return Err("Request ID already has different usage".into()),
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e.to_string()),
        }
        db.execute(
            "INSERT INTO api_spend(request_id,provider,model,input_tokens,output_tokens,cost,session_id,reasoning_effort,agent) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                entry.request_id,
                entry.provider,
                entry.model,
                entry.input_tokens,
                entry.output_tokens,
                entry.cost,
                entry.session_id,
                entry.reasoning_effort,
                entry.agent
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub fn summary(&self) -> Result<Vec<SpendRow>, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let mut query = db
            .prepare(
                "SELECT provider,
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cost), 0),
                        COUNT(*),
                        COUNT(DISTINCT session_id),
                        MIN(timestamp),
                        MAX(timestamp),
                        GROUP_CONCAT(DISTINCT reasoning_effort)
                 FROM api_spend
                 GROUP BY provider
                 ORDER BY provider",
            )
            .map_err(|e| e.to_string())?;
        let rows = query
            .query_map([], |r| {
                let efforts: Option<String> = r.get(8)?;
                Ok(SpendRow {
                    provider: r.get(0)?,
                    input_tokens: r.get(1)?,
                    output_tokens: r.get(2)?,
                    cost: r.get(3)?,
                    requests: r.get(4)?,
                    session_count: r.get(5)?,
                    first_seen: r.get(6)?,
                    last_seen: r.get(7)?,
                    reasoning_efforts: efforts
                        .map(|s| {
                            s.split(',')
                                .filter(|x| !x.is_empty())
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn resolve_window(window: &str) -> Result<(&'static str, String, String, usize), String> {
        let now = chrono::Utc::now();
        let today = now.date_naive();
        let to = today.format("%Y-%m-%d").to_string();
        match window {
            "today" => Ok(("today", to.clone(), to, 1)),
            "this_week" => {
                let from = today
                    .week(chrono::Weekday::Mon)
                    .first_day()
                    .format("%Y-%m-%d")
                    .to_string();
                let from_d = chrono::NaiveDate::parse_from_str(&from, "%Y-%m-%d").unwrap();
                let days = today.signed_duration_since(from_d).num_days() as usize + 1;
                Ok(("this_week", from, to, days.max(1)))
            }
            "this_month" => {
                let from = today
                    .with_day(1)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string();
                let from_d = chrono::NaiveDate::parse_from_str(&from, "%Y-%m-%d").unwrap();
                let days = today.signed_duration_since(from_d).num_days() as usize + 1;
                Ok(("this_month", from, to, days.max(1)))
            }
            "last_month" => {
                let first_this_month = today.with_day(1).unwrap();
                let last_of_prev = first_this_month - chrono::Duration::days(1);
                let first_of_prev = last_of_prev.with_day(1).unwrap();
                let from = first_of_prev.format("%Y-%m-%d").to_string();
                let to = last_of_prev.format("%Y-%m-%d").to_string();
                let days = last_of_prev.signed_duration_since(first_of_prev).num_days() as usize + 1;
                Ok(("last_month", from, to, days.max(1)))
            }
            _ => Err(format!("Unknown window: {window}")),
        }
    }

    pub fn series(&self, window: &str) -> Result<SpendSeries, String> {
        let (label, from, to, buckets) = Self::resolve_window(window)?;
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let from_ts = format!("{from}T00:00:00.000Z");
        let to_ts = format!("{to}T23:59:59.999Z");

        let mut stmt = db
            .prepare(
                "SELECT model,
                        COALESCE(reasoning_effort, '') AS effort,
                        substr(timestamp, 1, 10) AS day,
                        SUM(cost), COUNT(*)
                 FROM api_spend
                 WHERE timestamp >= ?1 AND timestamp <= ?2
                 GROUP BY model, effort, day
                 ORDER BY model, effort, day",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![from_ts, to_ts], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let days: Vec<String> = (0..buckets)
            .map(|i| {
                let d = chrono::NaiveDate::parse_from_str(&from, "%Y-%m-%d").unwrap()
                    + chrono::Duration::days(i as i64);
                d.format("%Y-%m-%d").to_string()
            })
            .collect();

        let mut index_of_day = std::collections::HashMap::new();
        for (i, d) in days.iter().enumerate() {
            index_of_day.insert(d.clone(), i);
        }

        let mut series_map: std::collections::BTreeMap<(String, String), ModelSeries> =
            std::collections::BTreeMap::new();
        let mut cost_by_day = vec![0.0_f64; days.len()];
        let mut tokens_by_day = vec![0_i64; days.len()];
        let mut total_cost = 0.0_f64;
        let mut total_requests = 0_i64;

        let mut tokens_stmt = db
            .prepare(
                "SELECT substr(timestamp,1,10), SUM(input_tokens+output_tokens)
                 FROM api_spend
                 WHERE timestamp >= ?1 AND timestamp <= ?2
                 GROUP BY substr(timestamp,1,10)",
            )
            .map_err(|e| e.to_string())?;
        let token_rows = tokens_stmt
            .query_map(params![from_ts, to_ts], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (day, count) in token_rows {
            if let Some(&i) = index_of_day.get(&day) {
                tokens_by_day[i] = count;
            }
        }

        for (model, effort, day, cost, requests) in rows {
            total_cost += cost;
            total_requests += requests;
            let Some(&i) = index_of_day.get(&day) else { continue };
            cost_by_day[i] += cost;
            let key = (model.clone(), effort.clone());
            let entry = series_map.entry(key).or_insert_with(|| ModelSeries {
                model: model.clone(),
                reasoning_effort: if effort.is_empty() { None } else { Some(effort.clone()) },
                total_cost: 0.0,
                total_requests: 0,
                points: days
                    .iter()
                    .map(|d| SeriesPoint { day: d.clone(), cost: 0.0, requests: 0 })
                    .collect(),
            });
            entry.total_cost += cost;
            entry.total_requests += requests;
            entry.points[i] = SeriesPoint {
                day: day.clone(),
                cost,
                requests,
            };
        }

        let mut series: Vec<ModelSeries> = series_map.into_values().collect();
        series.retain(|s| s.total_cost > 0.0 || s.total_requests > 0);
        series.sort_by(|a, b| {
            b.total_cost
                .partial_cmp(&a.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(SpendSeries {
            window: label.to_string(),
            from,
            to,
            total_cost,
            total_requests,
            days,
            series,
            tokens_by_day,
            cost_by_day,
        })
    }
}

#[tauri::command]
pub fn api_spend_summary(state: tauri::State<'_, SpendState>) -> Result<Vec<SpendRow>, String> {
    state.summary()
}
#[tauri::command]
pub fn api_spend_proxy_info(state: tauri::State<'_, SpendState>) -> Result<ProxyInfo, String> {
    state
        .info
        .lock()
        .map(|v| v.clone())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn api_spend_series(
    state: tauri::State<'_, SpendState>,
    window: String,
) -> Result<SpendSeries, String> {
    state.series(&window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_upgrade_backs_up_demo_and_preserves_reported_usage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api-spend.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE api_spend (
            id INTEGER PRIMARY KEY, request_id TEXT NOT NULL, provider TEXT NOT NULL,
            model TEXT NOT NULL, input_tokens INTEGER NOT NULL, output_tokens INTEGER NOT NULL,
            cost REAL NOT NULL, timestamp TEXT NOT NULL, source TEXT NOT NULL,
            UNIQUE(source,request_id));
            INSERT INTO api_spend VALUES(1,'demo','vertex','m',1,1,1,'2026-10-15','demo');
            INSERT INTO api_spend VALUES(2,'real','hermes','m',2,3,0.25,'2026-10-15','reported');");
        drop(db);
        let state = SpendState::open(path.clone()).unwrap();
        let rows = state.summary().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider, "hermes");
        assert_eq!(rows[0].cost, 0.25);
        assert_eq!(rows[0].input_tokens, 2);
        drop(state);
        assert_eq!(SpendState::open(path).unwrap().summary().unwrap().len(), 1);
        let backup = std::fs::read_dir(dir.path()).unwrap().filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().contains("pre-providers-")).unwrap();
        let db = Connection::open(backup.path()).unwrap();
        assert_eq!(db.query_row("SELECT COUNT(*) FROM api_spend", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    }

    fn entry(req: &str, model: &str, cost: f64, session: Option<&str>, effort: Option<&str>) -> UsageEntry {
        UsageEntry {
            request_id: req.into(),
            provider: "hermes".into(),
            model: model.into(),
            input_tokens: 100,
            output_tokens: 50,
            cost,
            session_id: session.map(str::to_string),
            reasoning_effort: effort.map(str::to_string),
            agent: Some("hermes".into()),
        }
    }

    #[test]
    fn persistence_and_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spend.db");
        let state = SpendState::open(path.clone()).unwrap();
        let e = entry("r1", "google/gemini-3.8-flash", 0.01, Some("s1"), Some("low"));
        assert!(state.record(&e).unwrap());
        assert!(!state.record(&e).unwrap());

        let mut e2 = e.clone();
        e2.cost = 0.02;
        assert!(state.record(&e2).is_err());

        let e3 = entry("r2", "google/gemini-3.8-flash", 0.005, Some("s1"), Some("low"));
        state.record(&e3).unwrap();

        let summary = state.summary().unwrap();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].requests, 2);
        assert_eq!(summary[0].session_count, 1);
    }

    #[test]
    fn rejects_unknown_provider_and_bad_effort() {
        let dir = tempfile::tempdir().unwrap();
        let state = SpendState::open(dir.path().join("spend.db")).unwrap();
        let mut e = entry("r1", "x", 0.01, None, None);
        e.provider = "claude".into();
        assert!(state.record(&e).is_err());

        let mut e = entry("r2", "x", 0.01, None, None);
        e.reasoning_effort = Some("ludicrous".into());
        assert!(state.record(&e).is_err());
    }

    #[test]
    fn accepts_configured_kinds_and_rejects_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let state = SpendState::open(dir.path().join("spend.db")).unwrap();
        let kinds = vec!["vertex".to_string()];

        let mut vertex = entry("v1", "google/gemini-3.8-flash", 0.01, None, None);
        vertex.provider = "vertex".into();
        assert!(state.record_allowing(&vertex, &kinds).unwrap());

        let mut devpass = entry("d1", "x", 0.01, None, None);
        devpass.provider = "devpass".into();
        assert!(state.record_allowing(&devpass, &kinds).is_err());
        assert!(state.record(&vertex).is_err());
    }

    #[test]
    fn series_resolves_windows_and_aggregates_per_model() {
        let dir = tempfile::tempdir().unwrap();
        let state = SpendState::open(dir.path().join("spend.db")).unwrap();
        let now = chrono::Utc::now();
        let today = now.date_naive().format("%Y-%m-%d").to_string();
        let yesterday = (now.date_naive() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();

        state
            .record(&UsageEntry {
                request_id: "t1".into(),
                provider: "hermes".into(),
                model: "google/gemini-3.8-flash".into(),
                input_tokens: 100,
                output_tokens: 50,
                cost: 0.001,
                session_id: Some("s1".into()),
                reasoning_effort: Some("low".into()),
                agent: Some("hermes".into()),
            })
            .unwrap();
        let db = state.db.lock().unwrap();
        db.execute(
            "INSERT INTO api_spend(request_id,provider,model,input_tokens,output_tokens,cost,timestamp,session_id,reasoning_effort,agent)
             VALUES('y1','hermes','google/gemini-3.8-flash',100,50,0.002,?1,'s2','low','hermes')",
            params![format!("{yesterday}T12:00:00.000Z")],
        )
        .unwrap();
        drop(db);

        let s = state.series("today").unwrap();
        assert_eq!(s.window, "today");
        assert_eq!(s.from, today);
        assert_eq!(s.to, today);
        assert!((s.total_cost - 0.001).abs() < 1e-9);
        assert_eq!(s.series.len(), 1);

        let s = state.series("this_week").unwrap();
        assert_eq!(s.window, "this_week");
        assert!(s.total_cost >= 0.001);

        let prev = (now.date_naive().with_day(1).unwrap() - chrono::Duration::days(15))
            .format("%Y-%m-%d")
            .to_string();
        let db = state.db.lock().unwrap();
        db.execute(
            "INSERT INTO api_spend(request_id,provider,model,input_tokens,output_tokens,cost,timestamp,session_id,reasoning_effort,agent)
             VALUES('p1','hermes','google/gemini-3.8-flash',100,50,0.005,?1,'s3','medium','hermes')",
            params![format!("{prev}T12:00:00.000Z")],
        )
        .unwrap();
        drop(db);

        let s = state.series("last_month").unwrap();
        assert_eq!(s.window, "last_month");
        assert!((s.total_cost - 0.005).abs() < 1e-9);
        assert!(s.series.iter().any(|m| m.reasoning_effort.as_deref() == Some("medium")));

        assert!(state.series("custom").is_err());
    }

    #[test]
    fn series_window_today_has_one_bucket() {
        let (label, from, to, days) = SpendState::resolve_window("today").unwrap();
        assert_eq!(label, "today");
        assert_eq!(from, to);
        assert_eq!(days, 1);
    }

    #[test]
    fn series_window_last_month_uses_full_previous_calendar_month() {
        let (_label, from, to, _days) = SpendState::resolve_window("last_month").unwrap();
        let from_d = chrono::NaiveDate::parse_from_str(&from, "%Y-%m-%d").unwrap();
        let to_d = chrono::NaiveDate::parse_from_str(&to, "%Y-%m-%d").unwrap();
        assert_eq!(from_d.with_day(1).unwrap(), from_d);
        assert!(to_d >= from_d);
    }
}