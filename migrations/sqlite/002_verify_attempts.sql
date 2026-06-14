-- migrations/sqlite/002_verify_attempts.sql
-- Track failed verification-code guesses so a pending code can be burned
-- after too many wrong attempts (brute-force resistance).
ALTER TABLE email_verifications ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
