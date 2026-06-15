-- migrations/sqlite/001_init.sql
-- CHECK constraints mirror Rust validation constants.

-- ─── Heavy auth (email + password accounts) ─────────────────────────
CREATE TABLE IF NOT EXISTS accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        VARCHAR(32) UNIQUE NOT NULL CHECK (length(username) >= 3 AND length(username) <= 32),
    email_hash      CHAR(64) UNIQUE NOT NULL,
    password_hash   VARCHAR(128) NOT NULL,
    tier            INTEGER NOT NULL DEFAULT 0,
    disabled        INTEGER NOT NULL DEFAULT 0,
    phone_hash      CHAR(64),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    last_login      TEXT
);

CREATE TABLE IF NOT EXISTS email_verifications (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    email_hash      CHAR(64) NOT NULL,
    username        VARCHAR(32) NOT NULL CHECK (length(username) >= 3),
    password_hash   VARCHAR(128) NOT NULL,
    code            CHAR(6) NOT NULL,
    expires_at      TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ─── Demo identity: name + 4-digit secret ───────────────────────────
-- The (name, secret) PAIR is the identity. Many people may share a name
-- as long as their secret differs. secret_hash = sha256(name + ':' + pin).
-- NOTE: a 4-digit PIN is demo-only; not real security.
CREATE TABLE IF NOT EXISTS persons (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            VARCHAR(64) NOT NULL CHECK (length(name) >= 1 AND length(name) <= 64),
    secret_hash     CHAR(64) NOT NULL,
    disabled        INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(name, secret_hash)
);

-- ─── Subject model ──────────────────────────────────────────────────
-- A submission/template is owned by a "subject", which is either an
-- 'account' (email/password) or a 'person' (demo PIN). subject_id is the id
-- within the corresponding table. This avoids conflating the two id spaces
-- (the bug where an account id was stored in a persons foreign key).

-- ─── Template registry ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS templates (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    receipt_no      VARCHAR(32) UNIQUE NOT NULL,
    subject_kind    VARCHAR(16) NOT NULL CHECK (subject_kind IN ('account', 'person')),
    subject_id      INTEGER NOT NULL,
    entity_name     VARCHAR(128),
    name            VARCHAR(128) NOT NULL CHECK (length(name) >= 3 AND length(name) <= 128),
    description     VARCHAR(512) CHECK (description IS NULL OR length(description) <= 512),
    fiscal_year     CHAR(4) NOT NULL CHECK (length(fiscal_year) = 4),
    raw_csv         TEXT NOT NULL CHECK (length(raw_csv) <= 512000),
    hidden          INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS template_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    template_id     INTEGER NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
    node_id         VARCHAR(32) NOT NULL CHECK (length(node_id) >= 3 AND length(node_id) <= 32),
    value           REAL NOT NULL CHECK (value >= 0)
);

-- ─── Submitted Tax Dollars (one per subject per fiscal year) ─────────
CREATE TABLE IF NOT EXISTS tax_dollars (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    receipt_token   VARCHAR(35) UNIQUE NOT NULL,
    subject_kind    VARCHAR(16) NOT NULL CHECK (subject_kind IN ('account', 'person')),
    subject_id      INTEGER NOT NULL,
    fiscal_year     CHAR(4) NOT NULL CHECK (length(fiscal_year) = 4),
    template_receipt_no VARCHAR(32) NOT NULL CHECK (length(template_receipt_no) >= 3 AND length(template_receipt_no) <= 32),
    raw_csv         TEXT NOT NULL CHECK (length(raw_csv) <= 512000),
    checksum        CHAR(71) NOT NULL CHECK (length(checksum) = 71),
    hidden          INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(subject_kind, subject_id, fiscal_year)
);

CREATE TABLE IF NOT EXISTS tax_dollar_allocations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tax_dollar_id   INTEGER NOT NULL REFERENCES tax_dollars(id) ON DELETE CASCADE,
    node_id         VARCHAR(32) NOT NULL CHECK (length(node_id) >= 3 AND length(node_id) <= 32),
    pct             REAL NOT NULL CHECK (pct >= 0.0 AND pct <= 1.0)
);
