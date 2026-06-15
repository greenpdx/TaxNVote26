// src/state.rs — AppState backed by SQLx (sqlite or postgres via AnyPool).

use sqlx::AnyPool;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use crate::mailer::Mailer;
use crate::models::*;

// ─── DB backend ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbBackend {
    Sqlite,
    Postgres,
}

impl DbBackend {
    pub fn from_url(url: &str) -> Result<Self, String> {
        if url.starts_with("sqlite:") {
            Ok(DbBackend::Sqlite)
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            Ok(DbBackend::Postgres)
        } else {
            Err(format!("DATABASE_URL must start with sqlite: or postgres: — got {url}"))
        }
    }
}

// ─── Rate limiter ────────────────────────────────────────────────

#[derive(Debug)]
pub struct RateLimiter {
    buckets: HashMap<(IpAddr, &'static str), Vec<Instant>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self { buckets: HashMap::new() }
    }

    /// Returns Ok(()) if allowed, Err(retry_after_secs) if rate-limited.
    pub fn check(&mut self, ip: IpAddr, endpoint: &'static str, max: usize, window_secs: u64) -> Result<(), u64> {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(window_secs);
        let key = (ip, endpoint);

        let entries = self.buckets.entry(key).or_default();
        entries.retain(|t| now.duration_since(*t) < window);

        if entries.len() >= max {
            let oldest = entries[0];
            let retry_after = window.as_secs() - now.duration_since(oldest).as_secs();
            return Err(retry_after.max(1));
        }

        entries.push(now);
        Ok(())
    }

    /// Periodic cleanup of expired entries.
    pub fn cleanup(&mut self, max_window_secs: u64) {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(max_window_secs);
        self.buckets.retain(|_, entries| {
            entries.retain(|t| now.duration_since(*t) < window);
            !entries.is_empty()
        });
    }
}

// ─── PoW challenge store ─────────────────────────────────────────

#[derive(Debug)]
pub struct ChallengeEntry {
    pub challenge: String,
    pub created: Instant,
}

#[derive(Debug)]
pub struct ChallengeStore {
    pending: HashMap<String, ChallengeEntry>,
}

impl ChallengeStore {
    pub fn new() -> Self {
        Self { pending: HashMap::new() }
    }

    pub fn issue(&mut self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let challenge: String = (0..CHALLENGE_LEN)
            .map(|_| format!("{:x}", rng.r#gen::<u8>() & 0x0f))
            .collect();

        // Cleanup expired while we're here
        let now = Instant::now();
        let ttl = std::time::Duration::from_secs(CHALLENGE_TTL_SECS);
        self.pending.retain(|_, e| now.duration_since(e.created) < ttl);

        self.pending.insert(challenge.clone(), ChallengeEntry {
            challenge: challenge.clone(),
            created: now,
        });
        challenge
    }

    /// Consume a challenge (single-use). Returns true if valid and not expired.
    pub fn consume(&mut self, challenge: &str) -> bool {
        let now = Instant::now();
        let ttl = std::time::Duration::from_secs(CHALLENGE_TTL_SECS);
        if let Some(entry) = self.pending.remove(challenge) {
            now.duration_since(entry.created) < ttl
        } else {
            false
        }
    }
}

// ─── App state ───────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub db: AnyPool,
    pub backend: DbBackend,
    #[allow(dead_code)] // surfaced for future handlers (e.g. file-backed assets)
    pub data_dir: PathBuf,
    pub jwt_secret: String,
    pub fiscal_year: String,
    /// JWT lifetime in seconds (configurable via JWT_TTL_HOURS).
    pub jwt_ttl_secs: i64,
    /// When true, derive the client IP from X-Forwarded-For (set by a trusted
    /// reverse proxy). When false, use the TCP peer address.
    pub trusted_proxy: bool,
    pub valid_node_ids: Arc<HashSet<String>>,
    pub rate_limiter: Arc<RwLock<RateLimiter>>,
    pub challenges: Arc<RwLock<ChallengeStore>>,
    pub mailer: Arc<dyn Mailer>,
    /// Cached aggregate results keyed by fiscal_year. Invalidated on submit
    /// (recompute-on-change); a present entry is the last computed display.
    pub aggregate_cache: Arc<RwLock<std::collections::HashMap<String, crate::models::AggregateResponse>>>,
    /// Runtime settings (admin-editable), cached from the `settings` table.
    pub settings: Arc<RwLock<HashMap<String, String>>>,
}

impl AppState {
    /// Build the state. Caller has already loaded .env and created the pool/mailer.
    pub fn new(
        db: AnyPool,
        backend: DbBackend,
        data_dir: PathBuf,
        jwt_secret: String,
        fiscal_year: String,
        jwt_ttl_secs: i64,
        trusted_proxy: bool,
        valid_node_ids: HashSet<String>,
        mailer: Arc<dyn Mailer>,
    ) -> Self {
        Self {
            db,
            backend,
            data_dir,
            jwt_secret,
            fiscal_year,
            jwt_ttl_secs,
            trusted_proxy,
            valid_node_ids: Arc::new(valid_node_ids),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new())),
            challenges: Arc::new(RwLock::new(ChallengeStore::new())),
            mailer,
            aggregate_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            settings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load all rows from the `settings` table into the in-memory cache.
    pub async fn reload_settings(&self) -> Result<(), sqlx::Error> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(&self.db).await?;
        let mut map = HashMap::new();
        for r in &rows {
            let k: String = r.try_get("key").unwrap_or_default();
            let v: String = r.try_get("value").unwrap_or_default();
            map.insert(k, v);
        }
        *self.settings.write().await = map;
        Ok(())
    }

    /// Get a setting, falling back to `default` when unset.
    pub async fn setting_or(&self, key: &str, default: bool) -> bool {
        match self.settings.read().await.get(key) {
            Some(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
            None => default,
        }
    }

    /// Get a string setting, falling back to `default` when unset or blank.
    pub async fn setting_str(&self, key: &str, default: &str) -> String {
        match self.settings.read().await.get(key) {
            Some(v) if !v.trim().is_empty() => v.clone(),
            _ => default.to_string(),
        }
    }

    /// Upsert a setting in the DB and refresh the cache.
    pub async fn set_setting(&self, key: &str, value: &str, by: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        // Portable upsert across sqlite/postgres.
        sqlx::query(&self.q(
            "INSERT INTO settings (key, value, updated_at, updated_by) VALUES (?, ?, ?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, updated_by = excluded.updated_by"
        ))
            .bind(key).bind(value).bind(&now).bind(by)
            .execute(&self.db).await?;
        self.settings.write().await.insert(key.to_string(), value.to_string());
        Ok(())
    }

    /// True if the given subject account/person is flagged disabled.
    pub async fn subject_disabled(&self, kind: &str, id: i64) -> Result<bool, sqlx::Error> {
        let table = if kind == crate::models::SUBJECT_ACCOUNT { "accounts" } else { "persons" };
        let row = sqlx::query(&self.q(&format!("SELECT disabled FROM {table} WHERE id = ? LIMIT 1")))
            .bind(id)
            .fetch_optional(&self.db).await?;
        match row {
            Some(r) => Ok(r.try_get::<i64, _>("disabled").unwrap_or(0) != 0),
            None => Ok(true), // unknown subject → treat as not authorized
        }
    }

    /// True if a token id has been revoked.
    pub async fn is_token_revoked(&self, jti: &str) -> Result<bool, sqlx::Error> {
        if jti.is_empty() { return Ok(false); }
        let row = sqlx::query(&self.q("SELECT 1 AS one FROM revoked_tokens WHERE jti = ? LIMIT 1"))
            .bind(jti)
            .fetch_optional(&self.db).await?;
        Ok(row.is_some())
    }

    /// Append an entry to the audit log. Best-effort: failures are logged, not
    /// propagated, so auditing never breaks the request it describes.
    #[allow(clippy::too_many_arguments)]
    pub async fn audit(
        &self,
        actor_kind: &str,
        actor_id: Option<i64>,
        action: &str,
        target_kind: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
        ip: Option<&str>,
    ) {
        let res = sqlx::query(&self.q(
            "INSERT INTO audit_log (actor_kind, actor_id, action, target_kind, target_id, detail, ip) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        ))
            .bind(actor_kind)
            .bind(actor_id)
            .bind(action)
            .bind(target_kind)
            .bind(target_id)
            .bind(detail)
            .bind(ip)
            .execute(&self.db).await;
        if let Err(e) = res {
            tracing::error!("audit write failed for action '{action}': {e}");
        }
    }

    /// Apply a sliding-window rate limit for `ip` on `endpoint`.
    /// Ok(()) if allowed, Err(retry_after_secs) if limited.
    pub async fn rate_limit(
        &self, ip: IpAddr, endpoint: &'static str, max: usize, window_secs: u64,
    ) -> Result<(), u64> {
        let mut rl = self.rate_limiter.write().await;
        rl.check(ip, endpoint, max, window_secs)
    }

    pub async fn next_template_receipt(&self) -> Result<String, sqlx::Error> {
        // Use COUNT + 1 so receipts are stable even across DB row deletions
        // in dev. Collisions are guarded by the UNIQUE constraint.
        let row: (i64,) = sqlx::query_as("SELECT COALESCE(MAX(id), 0) + 1 FROM templates")
            .fetch_one(&self.db).await?;
        let year = chrono::Utc::now().format("%Y");
        Ok(format!("TPL-{}-{:06}", year, row.0))
    }

    /// Translate `?` placeholders to `$N` form when the backend is Postgres.
    /// SQLite-style `?` passes through unchanged.
    /// Single-quoted string literals are skipped so embedded `?` is left alone.
    pub fn q(&self, sql: &str) -> String {
        if self.backend == DbBackend::Sqlite { return sql.to_string(); }
        let mut out = String::with_capacity(sql.len() + 8);
        let mut n = 1u32;
        let mut in_str = false;
        for c in sql.chars() {
            if c == '\'' { in_str = !in_str; out.push(c); }
            else if c == '?' && !in_str {
                out.push('$');
                out.push_str(&n.to_string());
                n += 1;
            } else { out.push(c); }
        }
        out
    }

    pub fn generate_td_receipt(&self) -> String {
        use rand::Rng;
        const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        let mut rng = rand::thread_rng();
        let token: String = (0..32).map(|_| BASE62[rng.r#gen_range(0..62)] as char).collect();
        format!("TD-{}", token)
    }
}

// ─── Node ID loader ──────────────────────────────────────────────

/// Load the valid node-ID set from data_dir/budauth.csv.
/// Returns Err on any failure — callers must surface to the user; node-ID
/// validation must never silently be skipped.
pub fn load_node_ids(data_dir: &str) -> Result<HashSet<String>, String> {
    let mut ids = HashSet::new();
    let csv_path = format!("{}/budauth.csv", data_dir);
    let csv_text = std::fs::read_to_string(&csv_path)
        .map_err(|e| format!("failed to read {csv_path}: {e}"))?;

    // Topic IDs always exist in the tree (the 9 fixed simple-form categories).
    for t in tnv_budget_tree::topics::TOPICS {
        ids.insert(format!("t:{}", t.id));
    }

    let mut lines = csv_text.lines();
    let _header = lines.next();
    for line in lines {
        let fields = parse_csv_line(line);
        if fields.len() < 5 { continue; }
        let a = fields[0].trim();
        let b = fields[2].trim();
        let c = fields[4].trim();
        // Same topic assignment the budget tree uses, so node IDs match exactly.
        let topic = tnv_budget_tree::topics::topic_for(a, b);
        if !a.is_empty() { ids.insert(format!("a:{}:{}", topic, a)); }
        if !a.is_empty() && !b.is_empty() { ids.insert(format!("b:{}:{}:{}", topic, a, b)); }
        if !a.is_empty() && !b.is_empty() && !c.is_empty() { ids.insert(format!("c:{}:{}:{}:{}", topic, a, b, c)); }
    }
    ids.insert("root".to_string());
    if ids.len() <= 1 {
        return Err(format!("{csv_path} contained no usable rows"));
    }
    Ok(ids)
}

/// Parse a CSV line respecting quoted fields (handles commas inside quotes).
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote ""
                    current.push('"');
                    chars.next();
                } else {
                    // End of quoted field
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else {
            match ch {
                ',' => {
                    fields.push(current.clone());
                    current.clear();
                }
                '"' => {
                    in_quotes = true;
                }
                _ => {
                    current.push(ch);
                }
            }
        }
    }
    fields.push(current);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let mut rl = RateLimiter::new();
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        assert!(rl.check(ip, "register", 3, 900).is_ok());
        assert!(rl.check(ip, "register", 3, 900).is_ok());
        assert!(rl.check(ip, "register", 3, 900).is_ok());
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let mut rl = RateLimiter::new();
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        assert!(rl.check(ip, "register", 2, 900).is_ok());
        assert!(rl.check(ip, "register", 2, 900).is_ok());
        let result = rl.check(ip, "register", 2, 900);
        assert!(result.is_err());
        let retry = result.unwrap_err();
        assert!(retry > 0 && retry <= 900);
    }

    #[test]
    fn test_rate_limiter_separate_ips() {
        let mut rl = RateLimiter::new();
        let ip1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        assert!(rl.check(ip1, "register", 1, 900).is_ok());
        assert!(rl.check(ip1, "register", 1, 900).is_err());
        // Different IP still allowed
        assert!(rl.check(ip2, "register", 1, 900).is_ok());
    }

    #[test]
    fn test_rate_limiter_separate_endpoints() {
        let mut rl = RateLimiter::new();
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        assert!(rl.check(ip, "register", 1, 900).is_ok());
        assert!(rl.check(ip, "register", 1, 900).is_err());
        // Different endpoint still allowed
        assert!(rl.check(ip, "login", 1, 900).is_ok());
    }

    #[test]
    fn test_challenge_issue_and_consume() {
        let mut cs = ChallengeStore::new();
        let c = cs.issue();
        assert_eq!(c.len(), CHALLENGE_LEN);
        // Consume succeeds once
        assert!(cs.consume(&c));
        // Second consume fails (single-use)
        assert!(!cs.consume(&c));
    }

    #[test]
    fn test_challenge_unknown_rejected() {
        let mut cs = ChallengeStore::new();
        assert!(!cs.consume("nonexistent"));
    }

    #[test]
    fn test_challenge_uniqueness() {
        let mut cs = ChallengeStore::new();
        let c1 = cs.issue();
        let c2 = cs.issue();
        assert_ne!(c1, c2);
    }

    // ─── CSV parser tests ────────────────────────────────────────

    #[test]
    fn test_parse_csv_simple() {
        let fields = parse_csv_line("a,b,c,d,e");
        assert_eq!(fields, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn test_parse_csv_quoted_comma() {
        let fields = parse_csv_line("001,\"Compensation of Members, Senate\",05");
        assert_eq!(fields, vec!["001", "Compensation of Members, Senate", "05"]);
    }

    #[test]
    fn test_parse_csv_escaped_quote() {
        let fields = parse_csv_line("a,\"say \"\"hi\"\"\",b");
        assert_eq!(fields, vec!["a", "say \"hi\"", "b"]);
    }

    #[test]
    fn test_parse_csv_empty_fields() {
        let fields = parse_csv_line(",,,,,,,,,,,,100,200");
        assert_eq!(fields.len(), 14);
        assert_eq!(fields[0], "");
        assert_eq!(fields[12], "100");
    }

    #[test]
    fn test_parse_csv_account_row() {
        let line = r#"001,Legislative Branch,05,Senate,0100,"Compensation of Members, Senate",00,000,801,Legislative functions,Mandatory,On-budget,24000,25000"#;
        let fields = parse_csv_line(line);
        assert_eq!(fields[0], "001");
        assert_eq!(fields[2], "05");
        assert_eq!(fields[4], "0100");
        assert_eq!(fields[5], "Compensation of Members, Senate");
        assert_eq!(fields.len(), 14);
    }

    #[test]
    fn test_parse_csv_parent_rows() {
        // Root row: all empty before year values
        let root = parse_csv_line(",,,,,,,,,,,,100,200");
        assert_eq!(root[0], "");
        assert_eq!(root[2], "");
        assert_eq!(root[4], "");

        // Agency row: only agency filled
        let agency = parse_csv_line("001,Legislative Branch,,,,,,,,,,,100,200");
        assert_eq!(agency[0], "001");
        assert_eq!(agency[2], "");
        assert_eq!(agency[4], "");

        // Bureau row: agency + bureau filled
        let bureau = parse_csv_line("001,Legislative Branch,05,Senate,,,,,,,,,100,200");
        assert_eq!(bureau[0], "001");
        assert_eq!(bureau[2], "05");
        assert_eq!(bureau[4], "");
    }

    #[test]
    fn test_load_node_ids_new_format() {
        let dir = "/tmp/tnv-test-nodeids";
        std::fs::create_dir_all(dir).ok();
        let csv = r#"Agency Code,Agency Name,Bureau Code,Bureau Name,Account Code,Account Name,Treasury Agency Code,CGAC Agency Code,Subfunction Code,Subfunction Title,BEA Category,On- or Off- Budget,2025
,,,,,,,,,,,,999
001,Legislative Branch,,,,,,,,,,,500
001,Legislative Branch,05,Senate,,,,,,,,,200
001,Legislative Branch,05,Senate,0100,"Compensation of Members, Senate",00,000,801,Legislative functions,Mandatory,On-budget,24000
001,Legislative Branch,05,Senate,0101,Officers and Employees,00,000,801,Legislative functions,Discretionary,On-budget,176000
"#;
        std::fs::write(format!("{}/budauth.csv", dir), csv).unwrap();

        let ids = load_node_ids(dir).unwrap();

        // Agency 001 (Legislative Branch) maps to the "oth" topic.
        assert!(ids.contains("root"));
        assert!(ids.contains("t:oth"));
        assert!(ids.contains("a:oth:001"));
        assert!(ids.contains("b:oth:001:05"));
        assert!(ids.contains("c:oth:001:05:0100"));
        assert!(ids.contains("c:oth:001:05:0101"));
        assert!(!ids.contains("a:oth:"));
        assert!(!ids.contains("b:oth::"));
        assert!(!ids.contains("c:oth:::"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_load_node_ids_missing_is_error() {
        let r = load_node_ids("/tmp/tnv-test-nodeids-does-not-exist-xyz");
        assert!(r.is_err());
    }

    #[test]
    fn test_db_backend_from_url() {
        assert_eq!(DbBackend::from_url("sqlite://x.db").unwrap(), DbBackend::Sqlite);
        assert_eq!(DbBackend::from_url("sqlite::memory:").unwrap(), DbBackend::Sqlite);
        assert_eq!(DbBackend::from_url("postgres://a@b/c").unwrap(), DbBackend::Postgres);
        assert_eq!(DbBackend::from_url("postgresql://a@b/c").unwrap(), DbBackend::Postgres);
        assert!(DbBackend::from_url("mysql://x").is_err());
    }
}
