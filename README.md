# TNV — Tax N Vote

A direct fiscal democracy platform where taxpayers allocate their "Tax Dollar"
across the federal budget. One taxpayer, one vote.

## Project Structure

```
tnv/
├── Cargo.toml                 # Rust workspace root
├── crates/
│   ├── tnv-budget-tree/       # Rust/WASM budget tree algorithm (35 tests)
│   └── tnv-server/            # Axum 0.8 REST API (40 tests)
├── frontend/                  # Vue 3 + TypeScript + Pinia UI
├── data/                      # budauth.csv + runtime store.json
├── migrations/                # SQL schema (SQLite/PostgreSQL)
└── docs/                      # Architecture plans and diagrams
```

## Prerequisites

- Rust 1.75+ (stable)
- Node.js 18+ / npm
- wasm-pack — `cargo install wasm-pack` (used by `frontend/`'s build scripts)
- `budauth.csv` in `data/` (CBO Budget Authority)
- `.env` at repo root — copy from `.env.example` and set at least `JWT_SECRET`,
  `DATABASE_URL`, and `FISCAL_YEAR`. Generate the secret with
  `openssl rand -hex 32`.

## Build & Run

### 1. Budget Tree (Rust library + WASM)

```bash
# Run tests (native)
cargo test -p tnv-budget-tree

# Build WASM standalone (normally `npm run build-wasm` from frontend/ handles this)
cd crates/tnv-budget-tree
wasm-pack build --target web --out-dir ../../frontend/src/wasm --features wasm
```

### 2. Server

```bash
# Run tests
cargo test -p tnv-server

# Run server (dev) — reads .env at repo root
cargo run -p tnv-server

# Required env (see .env.example):
#   JWT_SECRET    — JWT signing key, >=32 chars (server refuses to start otherwise)
#   DATABASE_URL  — sqlite://... or postgres://...
#   FISCAL_YEAR   — 4-digit year (e.g. 2027); tax-dollar submissions must match
# Optional:
#   DATA_DIR=data, BIND_ADDR=0.0.0.0:3000
#   SMTP_HOST/PORT/USER/PASS/FROM — empty SMTP_HOST → log-only mailer
#   RUST_LOG      — tracing filter
```

### 3. Frontend

```bash
cd frontend
cp .env.example .env   # set VITE_FISCAL_YEAR to match the server's FISCAL_YEAR
npm install
npm run dev            # dev server on :5173 (re-runs wasm-pack first)
npm run build          # production build → dist/ (re-runs wasm-pack first)
```

### Full Stack (dev)

Terminal 1:
```bash
DATA_DIR=data cargo run -p tnv-server
```

Terminal 2:
```bash
cd frontend && npm run dev
```

Vue dev server proxies `/api` to the Axum server on `:3000`.

## Architecture

### Registration Flow (anti-automation)

1. Client: `GET /api/auth/challenge` → receives PoW puzzle (SHA-256, 20 leading zero bits)
2. Client: solves puzzle in browser (~1-3s)
3. Client: `POST /api/auth/register` with `{ username, email, password, challenge, nonce }`
4. Server: rate limit (3/IP/15min) → field validation → PoW verify → email verification code
5. Client: `POST /api/auth/verify` → account created, JWT returned

### Budget Adjustment

- Tree structure is immutable after build (from CBO budauth.csv)
- User drags a slider → Rust/WASM computes proportional redistribution
- Only changesets returned to JS (~320 bytes per drag)
- Locked nodes are skipped; siblings absorb delta proportionally
- Changes confined within parent subtree

### API Endpoints

| Method | Path                          | Auth | Description              |
|--------|-------------------------------|------|--------------------------|
| GET    | /api/auth/challenge           | No   | PoW challenge            |
| POST   | /api/auth/register            | No   | Create account           |
| POST   | /api/auth/verify              | No   | Email verification       |
| POST   | /api/auth/login               | No   | Login → JWT              |
| GET    | /api/auth/me                  | JWT  | Current account          |
| GET    | /api/templates                | No   | List templates           |
| GET    | /api/templates/{receipt_no}   | No   | Download template CSV    |
| POST   | /api/templates                | JWT  | Upload template          |
| POST   | /api/taxdollar                | JWT  | Submit Tax Dollar        |
| GET    | /api/taxdollar/mine           | JWT  | List my Tax Dollars      |
| GET    | /api/taxdollar/{receipt_token}| No   | View by crypto receipt   |

## Test Summary

- **tnv-budget-tree**: 35 tests (adjust, lock, clamp, reset, template, CBO CSV)
- **tnv-server**: 40 tests (CSV parse, validation, auth, rate limit, PoW, length limits)

```bash
# Run all tests
cargo test --workspace
```

## License

Copyright © 2023-present Computado Rita. All rights reserved.
