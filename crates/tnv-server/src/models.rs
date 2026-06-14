// src/models.rs


use serde::{Deserialize, Serialize};

// ─── Length limits ────────────────────────────────────────────────

pub const USERNAME_MIN: usize = 3;
pub const USERNAME_MAX: usize = 32;
pub const EMAIL_MIN: usize = 5;
pub const EMAIL_MAX: usize = 254;
pub const PASSWORD_MIN: usize = 8;
pub const PASSWORD_MAX: usize = 128;
pub const TPL_NAME_MIN: usize = 3;
pub const TPL_NAME_MAX: usize = 128;
pub const TPL_DESC_MAX: usize = 512;
pub const FISCAL_YEAR_LEN: usize = 4;
pub const NODE_ID_MIN: usize = 3;
pub const NODE_ID_MAX: usize = 32;
pub const MAX_ENTRIES: usize = 5000;
pub const MAX_CSV_BYTES: usize = 512_000;
pub const CHECKSUM_LEN: usize = 71; // "sha256:" + 64 hex

// ─── Anti-automation ─────────────────────────────────────────────

pub const POW_DIFFICULTY: u32 = 20;          // leading zero bits in SHA-256
pub const CHALLENGE_TTL_SECS: u64 = 300;     // 5 min
pub const CHALLENGE_LEN: usize = 32;         // hex chars in challenge string

pub const RATE_REGISTER_MAX: usize = 3;
pub const RATE_REGISTER_WINDOW_SECS: u64 = 900;  // 15 min
pub const RATE_LOGIN_MAX: usize = 10;
pub const RATE_LOGIN_WINDOW_SECS: u64 = 900;
pub const RATE_VERIFY_MAX: usize = 10;
pub const RATE_VERIFY_WINDOW_SECS: u64 = 900;
pub const RATE_CHALLENGE_MAX: usize = 30;
pub const RATE_CHALLENGE_WINDOW_SECS: u64 = 900;
pub const RATE_IDENTIFY_MAX: usize = 5;
pub const RATE_IDENTIFY_WINDOW_SECS: u64 = 900;
pub const RATE_AGGREGATE_MAX: usize = 60;
pub const RATE_AGGREGATE_WINDOW_SECS: u64 = 60;
pub const RATE_SUBMIT_MAX: usize = 10;
pub const RATE_SUBMIT_WINDOW_SECS: u64 = 900;
pub const RATE_TEMPLATE_MAX: usize = 10;
pub const RATE_TEMPLATE_WINDOW_SECS: u64 = 900;

/// Max failed verification-code attempts before the pending row is burned.
pub const MAX_VERIFY_ATTEMPTS: i64 = 5;

// ─── Auth ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub challenge: String,
    pub nonce: String,
}

#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    pub challenge: String,
    pub difficulty: u32,
    pub expires_in_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

// ─── Demo identity: name + 4-digit secret ────────────────────────

pub const PIN_LEN: usize = 4;
pub const PERSON_NAME_MIN: usize = 1;
pub const PERSON_NAME_MAX: usize = 64;

#[derive(Debug, Deserialize)]
pub struct IdentifyRequest {
    pub name: String,
    pub secret: String, // 4-digit PIN
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: i64,
    pub username: String,
    pub tier: i32,
    pub created_at: String,
}

// ─── JWT Claims ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,        // account id
    pub username: String,
    pub tier: i32,
    pub exp: i64,        // expiry (unix timestamp)
}

// ─── Template ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TemplateSummary {
    pub receipt_no: String,
    pub name: String,
    pub entity_name: Option<String>,
    pub description: Option<String>,
    pub fiscal_year: String,
    pub entry_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct TemplateReceipt {
    pub receipt_no: String,
    pub name: String,
    pub created_at: String,
}

// ─── Tax Dollar ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TaxDollarReceipt {
    pub receipt_token: String,
    pub fiscal_year: String,
    pub created_at: String,
    pub replaced: bool,
}

#[derive(Debug, Serialize)]
pub struct TaxDollarSummary {
    pub receipt_token: String,
    pub fiscal_year: String,
    pub template_receipt_no: String,
    pub created_at: String,
}

// ─── CSV parsed structures ────────────────────────────────────────

#[derive(Debug)]
pub struct ParsedTemplate {
    pub name: String,
    pub entity_name: String,
    pub description: String,
    pub fiscal_year: String,
    pub entries: Vec<TemplateEntry>,
    pub raw_csv: String,
}

#[derive(Debug, Clone)]
pub struct TemplateEntry {
    pub node_id: String,
    pub value: f64,
}

#[derive(Debug)]
pub struct ParsedTaxDollar {
    pub fiscal_year: String,
    pub template_id: String,
    pub timestamp: String,
    pub checksum: String,
    pub allocations: Vec<Allocation>,
    pub raw_csv: String,
}

#[derive(Debug, Clone)]
pub struct Allocation {
    pub node_id: String,
    pub pct: f64,
}

// ─── Aggregate ("People's Budget") statistics ────────────────────

/// Fraction of submitters' values trimmed from EACH end for the trimmed mean.
pub const TRIM_FRACTION: f64 = 0.10;

#[derive(Debug, Clone, Serialize)]
pub struct NodeStat {
    pub node_id: String,
    pub count: usize,      // number of submitters in the sample
    pub mean: f64,
    pub median: f64,
    pub trimmed_mean: f64, // mean after dropping top/bottom TRIM_FRACTION
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregateResponse {
    pub fiscal_year: String,
    pub submission_count: usize,
    pub nodes: Vec<NodeStat>,
}
