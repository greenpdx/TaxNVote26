# syntax=docker/dockerfile:1
#
# TNV website image: builds the Vue/WASM frontend and the Rust API server, then
# ships a single small runtime image that serves the SPA and the /api endpoints.
#
#   docker build -t tnv-web:latest .
#   docker build --build-arg FISCAL_YEAR=2027 -t tnv-web:latest .
#
# TLS / reverse proxy / docker network are handled separately (see DOCKER.md);
# this image listens on plain HTTP :3000 and expects a proxy (e.g. Caddy) in front.

# ─────────────────────────── builder ───────────────────────────
FROM rust:1-bookworm AS builder

# Node.js (for the Vite frontend build) + wasm-pack (compiles the budget-tree
# crate to WebAssembly, invoked by `npm run build`).
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*
# cargo install guarantees wasm-pack lands on PATH ($CARGO_HOME/bin).
RUN cargo install wasm-pack

WORKDIR /src
COPY . .

# The frontend bakes VITE_FISCAL_YEAR at build time; it MUST match the server's
# runtime FISCAL_YEAR. Override with --build-arg FISCAL_YEAR=YYYY.
ARG FISCAL_YEAR=2027
ENV VITE_FISCAL_YEAR=${FISCAL_YEAR}

# Build the SPA (wasm-pack + vue-tsc + vite) → frontend/dist.
# npm install (not ci): the lockfile is intentionally not committed.
RUN cd frontend && npm install && npm run build

# Build the release server binary.
RUN cargo build --release -p tnv-server

# ─────────────────────────── runtime ───────────────────────────
FROM debian:bookworm-slim AS runtime

# ca-certificates: outbound TLS (SMTP, Postgres TLS). curl: container healthcheck.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /src/target/release/tnv-server ./tnv-server
COPY --from=builder /src/frontend/dist            ./static
COPY --from=builder /src/data/budauth.csv         ./data/budauth.csv

# Run unprivileged.
RUN useradd -r -u 10001 tnv && chown -R tnv:tnv /app
USER tnv

ENV STATIC_DIR=static \
    DATA_DIR=data \
    BIND_ADDR=0.0.0.0:3000
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://localhost:3000/api/health || exit 1

CMD ["./tnv-server"]
