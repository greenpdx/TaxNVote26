-- migrations/postgres/004_varchar.sql
-- The sqlx `Any` driver cannot decode the Postgres CHAR/BPCHAR type into a
-- String. Convert the fixed-length CHAR columns to VARCHAR so reads work via
-- the Any driver. Values are exact-length (hashes, codes, fiscal years), so no
-- padding is lost. SQLite treats CHAR as TEXT and is unaffected (no 004 there).
ALTER TABLE accounts            ALTER COLUMN email_hash  TYPE VARCHAR(64);
ALTER TABLE accounts            ALTER COLUMN phone_hash  TYPE VARCHAR(64);
ALTER TABLE email_verifications ALTER COLUMN email_hash  TYPE VARCHAR(64);
ALTER TABLE email_verifications ALTER COLUMN code        TYPE VARCHAR(6);
ALTER TABLE persons             ALTER COLUMN secret_hash TYPE VARCHAR(64);
ALTER TABLE templates           ALTER COLUMN fiscal_year TYPE VARCHAR(4);
ALTER TABLE tax_dollars         ALTER COLUMN fiscal_year TYPE VARCHAR(4);
ALTER TABLE tax_dollars         ALTER COLUMN checksum    TYPE VARCHAR(71);
