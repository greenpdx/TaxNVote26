-- migrations/postgres/001_init.sql
-- Belt-and-suspenders: CHECK constraints mirror Rust validation constants

CREATE TABLE IF NOT EXISTS accounts (
    id              BIGSERIAL PRIMARY KEY,
    username        VARCHAR(32) UNIQUE NOT NULL CHECK (length(username) >= 3 AND length(username) <= 32),
    email_hash      CHAR(64) UNIQUE NOT NULL,
    password_hash   VARCHAR(128) NOT NULL,
    tier            INTEGER NOT NULL DEFAULT 0,
    phone_hash      CHAR(64),
    created_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    last_login      TEXT
);

CREATE TABLE IF NOT EXISTS email_verifications (
    id              BIGSERIAL PRIMARY KEY,
    email_hash      CHAR(64) NOT NULL,
    username        VARCHAR(32) NOT NULL CHECK (length(username) >= 3),
    password_hash   VARCHAR(128) NOT NULL,
    code            CHAR(6) NOT NULL,
    expires_at      TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
);

CREATE TABLE IF NOT EXISTS persons (
    id              BIGSERIAL PRIMARY KEY,
    name            VARCHAR(64) NOT NULL CHECK (length(name) >= 1 AND length(name) <= 64),
    secret_hash     CHAR(64) NOT NULL,
    created_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    UNIQUE(name, secret_hash)
);

CREATE TABLE IF NOT EXISTS templates (
    id              BIGSERIAL PRIMARY KEY,
    receipt_no      VARCHAR(32) UNIQUE NOT NULL,
    person_id       BIGINT REFERENCES persons(id),
    uploader_id     BIGINT REFERENCES accounts(id),
    entity_name     VARCHAR(128),
    name            VARCHAR(128) NOT NULL CHECK (length(name) >= 3 AND length(name) <= 128),
    description     VARCHAR(512) CHECK (description IS NULL OR length(description) <= 512),
    fiscal_year     CHAR(4) NOT NULL CHECK (length(fiscal_year) = 4),
    raw_csv         TEXT NOT NULL CHECK (length(raw_csv) <= 512000),
    created_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
);

CREATE TABLE IF NOT EXISTS template_entries (
    id              BIGSERIAL PRIMARY KEY,
    template_id     BIGINT NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
    node_id         VARCHAR(32) NOT NULL CHECK (length(node_id) >= 3 AND length(node_id) <= 32),
    value           DOUBLE PRECISION NOT NULL CHECK (value >= 0)
);

CREATE TABLE IF NOT EXISTS tax_dollars (
    id              BIGSERIAL PRIMARY KEY,
    receipt_token   VARCHAR(35) UNIQUE NOT NULL,
    person_id       BIGINT NOT NULL REFERENCES persons(id),
    fiscal_year     CHAR(4) NOT NULL CHECK (length(fiscal_year) = 4),
    template_receipt_no VARCHAR(32) NOT NULL CHECK (length(template_receipt_no) >= 3 AND length(template_receipt_no) <= 32),
    raw_csv         TEXT NOT NULL CHECK (length(raw_csv) <= 512000),
    checksum        CHAR(71) NOT NULL CHECK (length(checksum) = 71),
    created_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    UNIQUE(person_id, fiscal_year)
);

CREATE TABLE IF NOT EXISTS tax_dollar_allocations (
    id              BIGSERIAL PRIMARY KEY,
    tax_dollar_id   BIGINT NOT NULL REFERENCES tax_dollars(id) ON DELETE CASCADE,
    node_id         VARCHAR(32) NOT NULL CHECK (length(node_id) >= 3 AND length(node_id) <= 32),
    pct             DOUBLE PRECISION NOT NULL CHECK (pct >= 0.0 AND pct <= 1.0)
);
