# TNV Axum 0.8 Server — Plan & Report

**Date:** May 21, 2026
**Project:** Tax N Vote — Backend Server
**Author:** Computado Rita / Shaun Savage

---

## 1. Purpose

Axum 0.8 server that:
- Serves the Vue 3 SPA and WASM binary as static files
- Manages budget templates (fetch and store)
- Receives and stores Tax Dollar submissions
- Validates both templates and Tax Dollars on receipt

---

## 2. Architecture

```
                    ┌──────────────────────────────┐
                    │  Browser / Phone             │
                    │  Vue 3 + WASM Budget Tree    │
                    └─────────────┬────────────────┘
                                  │ HTTPS
                    ┌─────────────▼────────────────┐
                    │  Axum 0.8 Server              │
                    │                               │
                    │  ┌─────────────────────────┐  │
                    │  │ Static File Service      │  │
                    │  │ Vue SPA + WASM pkg       │  │
                    │  └─────────────────────────┘  │
                    │                               │
                    │  ┌─────────────────────────┐  │
                    │  │ Template API             │  │
                    │  │ GET/POST /api/templates  │  │
                    │  └──────────┬──────────────┘  │
                    │             │                  │
                    │  ┌──────────▼──────────────┐  │
                    │  │ Tax Dollar API           │  │
                    │  │ POST /api/taxdollar      │  │
                    │  └──────────┬──────────────┘  │
                    │             │                  │
                    │  ┌──────────▼──────────────┐  │
                    │  │ Validation Layer         │  │
                    │  │ Template & TaxDollar     │  │
                    │  └──────────┬──────────────┘  │
                    │             │                  │
                    │  ┌──────────▼──────────────┐  │
                    │  │ Storage (filesystem)     │  │
                    │  │ data/templates/          │  │
                    │  │ data/taxdollars/         │  │
                    │  └─────────────────────────┘  │
                    └───────────────────────────────┘
```

---

## 3. Endpoints

### 3.1 Static Files

| Method | Path             | Description                        |
|--------|------------------|------------------------------------|
| GET    | /                | Vue SPA index.html                 |
| GET    | /assets/{*path}  | JS, CSS, WASM, source maps         |
| GET    | /pkg/{*path}     | WASM module files (tnv_budget_tree) |

Served via `tower_http::services::ServeDir`.
Fallback to index.html for Vue Router history mode.

### 3.2 Template API

| Method | Path                    | Body / Response                    |
|--------|-------------------------|------------------------------------|
| GET    | /api/templates          | → JSON array of template summaries |
| GET    | /api/templates/{id}     | → Full template JSON               |
| POST   | /api/templates          | ← Template JSON → validation + store |

**Template format** (what the client sends/receives):

```json
{
  "id": "fy2025-progressive",
  "name": "Progressive Budget FY2025",
  "description": "Prioritizes education and healthcare",
  "fiscal_year": "2021",
  "created_at": "2026-05-21T14:30:00Z",
  "entries": [
    { "id": "c:010:01:1000", "value": 280000 },
    { "id": "c:020:01:1000", "value": 55000 }
  ]
}
```

Sparse — only nodes that differ from the embedded default.
On POST, the server validates the template before storing.

### 3.3 Tax Dollar API

| Method | Path                    | Body / Response                        |
|--------|-------------------------|----------------------------------------|
| POST   | /api/taxdollar          | ← Tax Dollar JSON → validate + store   |
| GET    | /api/taxdollar/{id}     | → Stored Tax Dollar (for receipt)       |

**Tax Dollar format** (what the user submits):

```json
{
  "version": 1,
  "fiscal_year": "2021",
  "template_id": "default",
  "timestamp": "2026-05-21T14:30:00Z",
  "total": 1.00,
  "allocations": [
    { "id": "a:010", "pct": 0.3542 },
    { "id": "a:020", "pct": 0.1208 },
    { "id": "c:070:01:1000", "pct": 0.0045 }
  ],
  "checksum": "sha256:ab3f..."
}
```

- Percentages, not dollars — every user gets one Tax Dollar (1.00)
- Sparse diff from template — only changed nodes
- Checksum = SHA-256 of the sorted allocations array (id+pct pairs)
- Server validates: total sums to 1.00, checksum matches, all ids exist

On success, server returns the stored Tax Dollar with a server-assigned UUID.

### 3.4 Health / Info

| Method | Path            | Response                    |
|--------|-----------------|-----------------------------|
| GET    | /api/health     | `{ "status": "ok", "nodes": 1237 }` |

---

## 4. Data Structures (Rust)

```rust
// ─── Template ────────────────────────────────
#[derive(Serialize, Deserialize)]
struct TemplateEntry {
    id: String,
    value: f64,
}

#[derive(Serialize, Deserialize)]
struct Template {
    id: String,
    name: String,
    description: String,
    fiscal_year: String,
    created_at: DateTime<Utc>,
    entries: Vec<TemplateEntry>,
}

#[derive(Serialize)]
struct TemplateSummary {
    id: String,
    name: String,
    description: String,
    fiscal_year: String,
    entry_count: usize,
    created_at: DateTime<Utc>,
}

// ─── Tax Dollar ──────────────────────────────
#[derive(Serialize, Deserialize)]
struct Allocation {
    id: String,
    pct: f64,
}

#[derive(Serialize, Deserialize)]
struct TaxDollar {
    version: u32,
    fiscal_year: String,
    template_id: String,
    timestamp: DateTime<Utc>,
    total: f64,
    allocations: Vec<Allocation>,
    checksum: String,
}

#[derive(Serialize)]
struct TaxDollarReceipt {
    uuid: String,
    accepted_at: DateTime<Utc>,
    tax_dollar: TaxDollar,
}
```

---

## 5. Validation Rules

### 5.1 Template Validation (on POST /api/templates)

1. `id` must be non-empty, alphanumeric + hyphens, max 64 chars
2. `name` must be non-empty, max 256 chars
3. `fiscal_year` must match a valid year (e.g. "2021")
4. `entries` must be non-empty
5. Each entry `id` must be a valid node id format (`a:`, `b:`, `c:` prefix)
6. Each entry `value` must be >= 0
7. No duplicate entry ids
8. Template id must not already exist (no overwrite)

### 5.2 Tax Dollar Validation (on POST /api/taxdollar)

1. `version` must be 1
2. `fiscal_year` must be non-empty
3. `template_id` must reference an existing template (or "default")
4. `total` must equal 1.00 (within precision of 0.0001)
5. Each allocation `pct` must be >= 0.0 and <= 1.0
6. Sum of all allocation `pct` values must equal `total` (within 0.0001)
7. No duplicate allocation ids
8. `checksum` must match SHA-256 of canonical allocation string:
   - Sort allocations by id ascending
   - Concatenate: "id:pct,id:pct,..." (pct formatted to 6 decimals)
   - SHA-256 of that string, prefixed with "sha256:"
9. `timestamp` must be within 24 hours of server time (replay protection)

---

## 6. Storage

Phase 1: Filesystem (simple, no database dependency).

```
data/
├── templates/
│   ├── default.json
│   ├── fy2025-progressive.json
│   └── fy2025-conservative.json
└── taxdollars/
    ├── 2026-05/
    │   ├── a1b2c3d4-....json
    │   └── e5f6g7h8-....json
    └── 2026-06/
```

- Templates: one JSON file per template, named by id
- Tax Dollars: organized by year-month, named by UUID
- Phase 2 (future): PostgreSQL with the 35-table TNV schema

---

## 7. Middleware Stack

```rust
Router::new()
    // API routes
    .nest("/api", api_router())
    // Static files (Vue SPA + WASM)
    .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
    // Middleware
    .layer(CorsLayer::permissive())         // dev; tighten for prod
    .layer(TraceLayer::new_for_http())       // request logging
```

---

## 8. Application State

```rust
#[derive(Clone)]
struct AppState {
    /// Base directory for data storage
    data_dir: PathBuf,
    /// Set of valid node ids (loaded from budget CSV at startup)
    valid_node_ids: Arc<HashSet<String>>,
}
```

Node ids are loaded once at startup by parsing the same CBO CSV
that the WASM module uses. This lets the server validate that
template entries and tax dollar allocations reference real nodes.

---

## 9. File Structure

```
tnv-server/
├── Cargo.toml
├── src/
│   ├── main.rs          ← entry point, router, server startup
│   ├── state.rs         ← AppState, init, node id loading
│   ├── models.rs        ← Template, TaxDollar, Allocation structs
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── templates.rs ← GET/POST /api/templates
│   │   ├── taxdollar.rs ← POST/GET /api/taxdollar
│   │   └── health.rs    ← GET /api/health
│   └── validation.rs    ← Template + Tax Dollar validation
├── data/
│   ├── templates/
│   │   └── default.json ← ships with the server
│   └── taxdollars/
└── static/              ← Vue build output copied here
    ├── index.html
    ├── assets/
    └── pkg/             ← WASM module
```

---

## 10. Security Considerations

- **No auth in Phase 1** — this is a public voting tool
- **Rate limiting** (future): tower middleware, per-IP
- **Checksum verification** prevents tampered Tax Dollars
- **Timestamp window** prevents replay attacks
- **Input validation** on all endpoints; reject malformed data early
- **CORS** locked to specific origin in production
- **No PII** — Tax Dollars are anonymous by design
- **Filesystem permissions** — data/ directory not served by static handler

---

## 11. Implementation Phases

| Phase | What                            | Status    |
|-------|---------------------------------|-----------|
| 1     | Plan & report                   | ✅ This document |
| 2     | Core server: static + health    | ⬜ Next   |
| 3     | Template API + validation       | ⬜        |
| 4     | Tax Dollar API + validation     | ⬜        |
| 5     | Wire to Vue app                 | ⬜        |
| 6     | Integration testing             | ⬜        |
| 7     | Production hardening (TLS, rate limit, CORS) | ⬜ |

---

## 12. Dependencies

| Crate              | Version | Purpose                           |
|--------------------|---------|-----------------------------------|
| axum               | 0.8     | Web framework                     |
| tokio              | 1.x     | Async runtime                     |
| tower-http         | 0.6     | Static files, CORS, tracing       |
| serde / serde_json | 1.x     | JSON serialization                |
| chrono             | 0.4     | Timestamps                        |
| sha2               | 0.10    | Checksum computation              |
| hex                | 0.4     | Hex encoding for checksums        |
| uuid               | 1.x     | Tax Dollar receipt IDs            |
| tracing            | 0.1     | Structured logging                |
| tracing-subscriber | 0.3     | Log output                        |

All compatible with Rust 1.75 / axum 0.8.x.

---

## 13. Open Questions

1. **Auth**: Should template uploads require an API key? (Recommend: yes for production, skip for MVP)
2. **Aggregation**: Should the server compute aggregate Tax Dollar results? (Recommend: future phase, separate service)
3. **Database**: When to move from filesystem to PostgreSQL? (Recommend: when Tax Dollar volume exceeds ~10k/month)
4. **Rate limiting**: Requests per minute per IP? (Recommend: 60 req/min for API, unlimited for static)
