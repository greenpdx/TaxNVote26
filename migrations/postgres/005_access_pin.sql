-- migrations/postgres/005_access_pin.sql
-- Optional per-submission access PIN that gates the public link before the data
-- is released. Hash = sha256(receipt_token + ':' + pin); the receipt token is
-- the (unguessable) salt. NULL = no PIN set.
ALTER TABLE tax_dollars ADD COLUMN access_pin_hash VARCHAR(64);
