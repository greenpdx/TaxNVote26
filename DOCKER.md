# TNV — Docker package (website)

This packages the **website** as a single container: it builds the Vue/WASM
frontend and the Rust API server, then serves the SPA and the `/api` endpoints
on plain HTTP **:3000**.

> **Scope:** TLS, the reverse proxy (Caddy), and the shared docker network are
> handled in a **separate session/compose**. This image is the app only — it
> expects a proxy in front. The handoff is at the bottom.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage build → small runtime image (binary + SPA + `budauth.csv`) |
| `.dockerignore` | Keeps the build context small/reproducible |
| `docker-compose.yml` | Runs the `app` service on an internal network |
| `.env.docker.example` | Environment template — **copy to `.env.docker` and edit** |

## Build & run

```bash
cp .env.docker.example .env.docker     # then edit the values (see below)
docker compose up -d --build
# or, image only:
docker build --build-arg FISCAL_YEAR=2027 -t tnv-web:latest .
```

The container is reachable on the internal `tnv-web` network as `app:3000`.
For a quick local check without a proxy, uncomment the `ports:` block in
`docker-compose.yml` (`127.0.0.1:3000:3000`) and open `http://127.0.0.1:3000`.

## Details to modify (`.env.docker`)

| Variable | What to set |
|----------|-------------|
| `JWT_SECRET` | **Fresh** secret per environment: `openssl rand -hex 32` |
| `DATABASE_URL` | Your Postgres: `postgres://USER:PASS@HOST:5432/DBNAME` |
| `FISCAL_YEAR` | 4-digit year — **must equal** the `FISCAL_YEAR` build arg |
| `BOOTSTRAP_ADMIN_EMAIL` | An already-registered account to promote to admin on start |
| `SMTP_*` | Email delivery for registration codes (empty `SMTP_HOST` = log-only) |
| `ENABLE_DEMO_IDENTITY` | Keep `false` in production |
| `ALLOWED_ORIGINS` | Leave empty for same-origin; set only for a cross-origin frontend |

> **Fiscal year:** the SPA bakes `VITE_FISCAL_YEAR` at build time. If you change
> `FISCAL_YEAR`, rebuild with `--build-arg FISCAL_YEAR=YYYY` (and the compose
> `args.FISCAL_YEAR`) so the frontend and server agree.

## What's in the image

- `/app/tnv-server` — release binary (rustls; no OpenSSL needed)
- `/app/static` — built SPA (served by the server; `STATIC_DIR=static`)
- `/app/data/budauth.csv` — required dataset (`DATA_DIR=data`)
- Runs as the unprivileged `tnv` user; `HEALTHCHECK` hits `/api/health`
  (which pings the DB → unhealthy if the database is unreachable).

## Database & migrations

The app **auto-runs migrations** at startup against `DATABASE_URL`. It does not
host Postgres — point `DATABASE_URL` at your instance and ensure the database
exists. First admin: register through the app, then set `BOOTSTRAP_ADMIN_EMAIL`
(or `docker compose exec app ./tnv-server admin promote <email>`).

## Handoff: TLS / reverse proxy / network (separate session)

This container speaks plain HTTP on `:3000` and is **not published** to the host
(only `expose`d). The proxy session should:

1. **Share the network.** Attach the proxy to the `tnv-web` network and reach
   the app as `http://app:3000`. If the proxy owns the network, make it
   `external: true` here.
2. **Terminate TLS** (e.g. Caddy automatic Let's Encrypt) and `reverse_proxy app:3000`.
3. **Set `X-Forwarded-For`.** The app already runs with `TRUSTED_PROXY=true`
   (see `docker-compose.yml`) and reads the **rightmost** XFF entry — make sure
   the proxy appends it. `TRUSTED_PROXY` must only be true behind a trusted proxy.
4. **Add security headers** at the proxy: HSTS, CSP, `X-Content-Type-Options:
   nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy`.
5. Keep the app bound to the internal network only (do not publish `:3000`
   publicly).

Backups (`pg_dump`) for the external Postgres are likewise out of scope for this
app container and belong with the database/infra setup.
