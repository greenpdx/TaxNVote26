-- migrations/postgres/003_admin.sql
-- Admin layer: audit trail, runtime settings, JWT revocation list.

CREATE TABLE IF NOT EXISTS audit_log (
    id              BIGSERIAL PRIMARY KEY,
    ts              TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    actor_kind      VARCHAR(16) NOT NULL,   -- account | person | system | anon
    actor_id        BIGINT,                 -- id within the actor's table (NULL for system/anon)
    action          VARCHAR(64) NOT NULL,   -- e.g. login, register, submit, admin.user.disable
    target_kind     VARCHAR(32),
    target_id       VARCHAR(64),
    detail          TEXT,                   -- optional JSON
    ip              VARCHAR(64)
);
CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_log(ts);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);

CREATE TABLE IF NOT EXISTS settings (
    key             VARCHAR(64) PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    updated_by      VARCHAR(64)
);

-- Revoked JWT ids (jti). A token is rejected if its jti is present and not
-- yet past expiry; rows can be pruned once expires_at has passed.
CREATE TABLE IF NOT EXISTS revoked_tokens (
    jti             VARCHAR(64) PRIMARY KEY,
    expires_at      BIGINT NOT NULL,        -- unix timestamp (token exp)
    revoked_at      TEXT NOT NULL DEFAULT to_char((now() at time zone 'utc'), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
);
