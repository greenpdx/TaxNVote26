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
| `ALLOWED_ORIGINS` | Leave empty for same-origin; set only for a cross-origin frontend |

> **Fiscal year:** the SPA bakes `VITE_FISCAL_YEAR` at build time. If you change
> `FISCAL_YEAR`, rebuild with `--build-arg FISCAL_YEAR=YYYY` (and the compose
> `args.FISCAL_YEAR`) so the frontend and server agree.

## Build variant: `full` vs `demo`

The image is built in one of two flavors, selected by the `VARIANT` build arg
(default `full`). It compiles a single source two ways — the SPA and the server
are always kept in sync because `VARIANT` drives both halves:

| Variant | Public sign-in | Registration | Use |
|---------|----------------|--------------|-----|
| `full` (default) | Email + password | Self-service (email verification) | Permanent public site |
| `demo` | Name + 4-digit PIN | None (find-or-create on sign-in) | Conference; admin still logs in by email |

The permanent site uses `docker-compose.yml` (image `tnv-web`). The conference
build uses `docker-compose.demo.yml` (image `tnv-demo`) and runs only while the
conference is on; the two are independent and can run side by side (give them
separate databases):

```sh
cp .env.docker.demo.example .env.docker.demo   # then edit
docker compose -f docker-compose.demo.yml up -d --build
```

To build the demo image by hand: `docker build --build-arg VARIANT=demo -t tnv-demo:latest .`

## Transferring to a host

Three ways to get the two images (`tnv-web`, `tnv-demo`) onto a deployment host.
The images are self-contained, but the compose + `.env.docker*` files are not —
copy those separately whichever option you pick.

### A — `docker save` → SSH → `docker load` (no registry)

Best for a one-off transfer. Build locally, then stream both images over SSH:

```sh
docker build --build-arg VARIANT=full -t tnv-web:latest  .
docker build --build-arg VARIANT=demo -t tnv-demo:latest .

docker save tnv-web:latest tnv-demo:latest | gzip | \
  ssh user@host 'gunzip | docker load'

# ship compose + your real (non-.example) env files
scp docker-compose.yml docker-compose.demo.yml user@host:/opt/tnv/
scp .env.docker .env.docker.demo user@host:/opt/tnv/
```

On the host, use `--no-build` so compose uses the loaded images instead of
rebuilding (both compose files carry a `build:` section):

```sh
cd /opt/tnv
docker compose up -d --no-build
docker compose -f docker-compose.demo.yml up -d --no-build
```

### B — Build on the host from source

Skip image transfer; build where it runs. The host needs the Rust+Node build
layers (slower first build, more disk), but it's reproducible from git:

```sh
git clone <repo> /opt/tnv && cd /opt/tnv
cp .env.docker.example .env.docker            # edit
cp .env.docker.demo.example .env.docker.demo  # edit
docker compose up -d --build
docker compose -f docker-compose.demo.yml up -d --build
```

### C — Registry (repeated deploys / multiple hosts / CI)

```sh
docker tag  tnv-web:latest  registry.example.com/tnv-web:latest
docker tag  tnv-demo:latest registry.example.com/tnv-demo:latest
docker push registry.example.com/tnv-web:latest
docker push registry.example.com/tnv-demo:latest
```

Point each compose file's `image:` at the registry path, `docker login` on the
host, then `docker compose pull && docker compose up -d`.

> **Watch out:**
> - The two builds need **separate Postgres databases** (the demo env already
>   points at `tnv_demo`) so conference submissions never mix with real data.
> - `FISCAL_YEAR` is **baked into the SPA at build time.** With option A (save/load)
>   it's already fixed — don't change it on the host without rebuilding.
> - TLS/reverse proxy is separate (see the handoff below); both containers only
>   `expose` `:3000` internally.

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
