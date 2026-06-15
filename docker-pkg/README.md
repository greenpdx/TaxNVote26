# TNV website — prebuilt Docker package

Self-contained, **runtime-only** package: the image is assembled from prebuilt
artifacts, so the build host needs **only Docker** — no Rust, Node, or wasm-pack,
and no source compilation.

## Contents

| Path | What it is |
|------|------------|
| `tnv-server` | Prebuilt release binary — **Debian bookworm / glibc 2.36, linux/amd64** |
| `static/` | Built SPA (fiscal year **2027** baked in) |
| `data/budauth.csv` | Required dataset |
| `Dockerfile` | Runtime image (only `COPY` + `apt`; no compiler) |
| `docker-compose.yml` | Runs the app on an internal network |
| `.env.docker.example` | Environment template → copy to `.env.docker` |

> **Platform:** the binary is dynamically linked against glibc 2.36 for
> **x86_64**. It runs on `debian:bookworm-slim` (matched) and Debian 12 / recent
> Ubuntu hosts. It will **not** run on Alpine (musl) or arm64 — for those, rebuild
> from source (see the repo's root `Dockerfile`).

## Run

```bash
cp .env.docker.example .env.docker     # then edit JWT_SECRET, DATABASE_URL, ...
docker compose up -d --build           # "build" just copies files into the image
```

Reachable on the internal `tnv-web` network as `app:3000`. For a quick local
check without a proxy, uncomment the `ports:` block (`127.0.0.1:3000:3000`).

## Details to set (`.env.docker`)

- `JWT_SECRET` — fresh per environment: `openssl rand -hex 32`
- `DATABASE_URL` — your Postgres `postgres://USER:PASS@HOST:5432/DBNAME`
- `FISCAL_YEAR` — keep **2027** (matches the baked SPA) unless you rebuild the frontend
- `BOOTSTRAP_ADMIN_EMAIL` — an existing account to promote to admin on start
- `SMTP_*` — email for registration codes (empty `SMTP_HOST` = log-only)

The app **auto-runs DB migrations** at startup. First admin: register via the
app, then set `BOOTSTRAP_ADMIN_EMAIL` (or
`docker compose exec app ./tnv-server admin promote <email>`).

## Handoff: TLS / reverse proxy / network (separate setup)

This container speaks plain HTTP on `:3000` and is **not published** to the host
(only `expose`d). The proxy setup should:

1. Attach the proxy (e.g. Caddy) to the `tnv-web` network and `reverse_proxy app:3000`.
2. Terminate TLS (automatic Let's Encrypt).
3. Append `X-Forwarded-For` — the app runs with `TRUSTED_PROXY=true` and reads
   the rightmost entry. Only keep `TRUSTED_PROXY=true` when actually behind a
   trusted proxy.
4. Add security headers: HSTS, CSP, `X-Content-Type-Options: nosniff`,
   `X-Frame-Options: DENY`, `Referrer-Policy`.
5. Keep `:3000` internal — do not publish it publicly.

## Rebuilding the artifacts (build-capable machine only)

From the repo root, the source `Dockerfile` builds everything from scratch, or:

```bash
cargo build --release -p tnv-server          # -> target/release/tnv-server
cd frontend && VITE_FISCAL_YEAR=2027 npm run build   # -> frontend/dist (= static/)
```
