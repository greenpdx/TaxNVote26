# syntax=docker/dockerfile:1
#
# Runtime-only image — NO compilation. It only copies prebuilt artifacts, so the
# build host needs nothing but Docker (no Rust, no Node, no wasm-pack).
#
# Ships:
#   tnv-server         prebuilt release binary (Debian bookworm / glibc 2.36, x86_64)
#   static/            built SPA (fiscal year baked in: 2027)
#   data/budauth.csv   required dataset
#
#   docker build -t tnv-web:latest .
#
# Plain HTTP on :3000 — put a TLS reverse proxy in front (see README.md).

FROM debian:bookworm-slim

# ca-certificates: outbound TLS (SMTP, Postgres TLS). curl: healthcheck.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY tnv-server ./tnv-server
COPY static ./static
COPY data/budauth.csv ./data/budauth.csv

RUN useradd -r -u 10001 tnv && chmod +x ./tnv-server && chown -R tnv:tnv /app
USER tnv

ENV STATIC_DIR=static \
    DATA_DIR=data \
    BIND_ADDR=0.0.0.0:3000
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://localhost:3000/api/health || exit 1

CMD ["./tnv-server"]
