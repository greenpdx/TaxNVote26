// In-crate integration tests for the admin layer + subject model, run against
// an in-memory SQLite database by calling the handlers directly.
#![cfg(test)]

use axum::extract::{Path, Query, State};
use axum::Json;
use sqlx::any::AnyPoolOptions;

use crate::extract::ClientIp;
use crate::handlers::{admin, aggregate, taxdollar};
use crate::mailer::LogMailer;
use crate::models::*;
use crate::state::{AppState, DbBackend};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Arc;

fn claims(sub: i64, kind: &str, tier: i32) -> Claims {
    Claims {
        sub,
        username: format!("{kind}{sub}"),
        tier,
        kind: kind.to_string(),
        jti: String::new(),
        exp: chrono::Utc::now().timestamp() + 3600,
    }
}

fn ip() -> ClientIp {
    ClientIp(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

async fn test_state() -> AppState {
    sqlx::any::install_default_drivers();
    // max_connections(1) keeps a single shared in-memory database alive.
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../../migrations/sqlite").run(&pool).await.unwrap();

    let mut ids = HashSet::new();
    ids.insert("t:def".to_string());
    let state = AppState::new(
        pool,
        DbBackend::Sqlite,
        PathBuf::from("data"),
        "test-secret-test-secret-test-secret".to_string(),
        "2027".to_string(),
        3600,
        false,
        false,
        ids,
        Arc::new(LogMailer::new()),
    );
    state.reload_settings().await.unwrap();
    state
}

async fn seed_account(state: &AppState, username: &str, email_h: &str, tier: i32) -> i64 {
    let row = sqlx::query(&state.q(
        "INSERT INTO accounts (username, email_hash, password_hash, tier, disabled) \
         VALUES (?, ?, ?, ?, 0) RETURNING id",
    ))
    .bind(username)
    .bind(email_h)
    .bind("x")
    .bind(tier)
    .fetch_one(&state.db)
    .await
    .unwrap();
    use sqlx::Row;
    row.try_get("id").unwrap()
}

async fn seed_person(state: &AppState, name: &str, secret_h: &str) -> i64 {
    let row = sqlx::query(&state.q(
        "INSERT INTO persons (name, secret_hash, disabled) VALUES (?, ?, 0) RETURNING id",
    ))
    .bind(name)
    .bind(secret_h)
    .fetch_one(&state.db)
    .await
    .unwrap();
    use sqlx::Row;
    row.try_get("id").unwrap()
}

#[tokio::test]
async fn admin_can_list_users_and_disable() {
    let state = test_state().await;
    let admin_id = seed_account(&state, "boss", &"a".repeat(64), ADMIN_TIER).await;
    let person_id = seed_person(&state, "alice", &"b".repeat(64)).await;
    let admin = claims(admin_id, SUBJECT_ACCOUNT, ADMIN_TIER);

    // List sees both the account and the person.
    let users = admin::list_users(
        State(state.clone()),
        claims(admin_id, SUBJECT_ACCOUNT, ADMIN_TIER),
        Query(admin::ListQuery { q: None, limit: None, action: None }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(users.len(), 2);

    // Disable the person, then the subject is reported disabled (immediate).
    let _ = admin::disable_user(
        State(state.clone()),
        ip(),
        admin,
        Path((SUBJECT_PERSON.to_string(), person_id)),
    )
    .await
    .unwrap();
    assert!(state.subject_disabled(SUBJECT_PERSON, person_id).await.unwrap());

    // A non-admin is rejected by the handler's own guard (defense in depth).
    let denied = admin::list_users(
        State(state.clone()),
        claims(person_id, SUBJECT_PERSON, 0),
        Query(admin::ListQuery { q: None, limit: None, action: None }),
    )
    .await;
    assert!(denied.is_err());
}

#[tokio::test]
async fn config_set_is_audited_and_visible() {
    let state = test_state().await;
    let admin_id = seed_account(&state, "boss", &"c".repeat(64), ADMIN_TIER).await;

    let _ = admin::set_config(
        State(state.clone()),
        ip(),
        claims(admin_id, SUBJECT_ACCOUNT, ADMIN_TIER),
        Path("registration_open".to_string()),
        Json(SetSettingRequest { value: "false".to_string() }),
    )
    .await
    .unwrap();

    // Setting is reflected through the cache and the config endpoint.
    assert!(!state.setting_or("registration_open", true).await);
    let cfg = admin::get_config(State(state.clone()), claims(admin_id, SUBJECT_ACCOUNT, ADMIN_TIER))
        .await
        .unwrap()
        .0;
    assert!(cfg.iter().any(|s| s.key == "registration_open" && s.value == "false"));

    // The change was audited.
    let entries = admin::list_audit(
        State(state.clone()),
        claims(admin_id, SUBJECT_ACCOUNT, ADMIN_TIER),
        Query(admin::ListQuery { q: None, limit: None, action: Some("admin.config.set".to_string()) }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].target_id.as_deref(), Some("registration_open"));

    // Rejects keys outside the allowlist (never touches secrets).
    let bad = admin::set_config(
        State(state.clone()),
        ip(),
        claims(admin_id, SUBJECT_ACCOUNT, ADMIN_TIER),
        Path("jwt_secret".to_string()),
        Json(SetSettingRequest { value: "leak".to_string() }),
    )
    .await;
    assert!(bad.is_err());
}

#[tokio::test]
async fn subject_model_submission_roundtrips() {
    let state = test_state().await;
    let person_id = seed_person(&state, "alice", &"d".repeat(64)).await;

    // A person and an account could share the same numeric id; the subject_kind
    // keeps their submissions distinct (the bug this model fixes).
    let acct_id = seed_account(&state, "alice2", &"e".repeat(64), 0).await;

    use sqlx::Row;
    for (kind, id) in [(SUBJECT_PERSON, person_id), (SUBJECT_ACCOUNT, acct_id)] {
        let td = sqlx::query(&state.q(
            "INSERT INTO tax_dollars (receipt_token, subject_kind, subject_id, fiscal_year, template_receipt_no, raw_csv, checksum) \
             VALUES (?, ?, ?, '2027', 'default', 'x', ?) RETURNING id",
        ))
        .bind(state.generate_td_receipt())
        .bind(kind)
        .bind(id)
        .bind(format!("sha256:{}", "0".repeat(64)))
        .fetch_one(&state.db)
        .await
        .unwrap();
        let td_id: i64 = td.try_get("id").unwrap();
        sqlx::query(&state.q(
            "INSERT INTO tax_dollar_allocations (tax_dollar_id, node_id, pct) VALUES (?, 't:def', 1.0)",
        ))
        .bind(td_id)
        .execute(&state.db)
        .await
        .unwrap();
    }

    // Each subject sees only their own submission.
    let mine = taxdollar::my_taxdollars(State(state.clone()), claims(person_id, SUBJECT_PERSON, 0))
        .await
        .unwrap()
        .0;
    assert_eq!(mine.len(), 1);

    // Aggregate counts both submissions for the year.
    let agg = aggregate::aggregate(
        State(state.clone()),
        ip(),
        Query(aggregate::AggQuery { fiscal_year: None }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(agg.submission_count, 2);
}
