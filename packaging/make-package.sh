#!/usr/bin/env bash
# Build both variants (release binary + SPA) and assemble the portable install
# tarball under dist-pkg/. Run from a machine with the Rust + Node toolchain:
#
#   ./packaging/make-package.sh
#   FISCAL_YEAR=2028 ./packaging/make-package.sh   # override the baked-in year
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

VERSION="$(grep -m1 '^version' crates/tnv-server/Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
ARCH="$(uname -m)"
FISCAL_YEAR="${FISCAL_YEAR:-2027}"
NAME="tnv-${VERSION}-linux-${ARCH}"
OUT="$REPO/dist-pkg"
STAGE="$OUT/$NAME"

rm -rf "$STAGE"
install -d "$STAGE/full" "$STAGE/demo" "$STAGE/data" "$STAGE/systemd" "$STAGE/env"

echo "== building full server + SPA (FISCAL_YEAR=$FISCAL_YEAR) =="
cargo build --release -p tnv-server
( cd frontend && VITE_AUTH_MODE=full VITE_FISCAL_YEAR="$FISCAL_YEAR" npm run build >/dev/null )
cp target/release/tnv-server "$STAGE/full/tnv-server"
cp -r frontend/dist "$STAGE/full/static"

echo "== building demo server + SPA =="
cargo build --release -p tnv-server --no-default-features --features demo
( cd frontend && VITE_AUTH_MODE=demo VITE_FISCAL_YEAR="$FISCAL_YEAR" npm run build >/dev/null )
cp target/release/tnv-server "$STAGE/demo/tnv-server"
cp -r frontend/dist "$STAGE/demo/static"

echo "== assembling shared assets =="
cp data/budauth.csv          "$STAGE/data/"
cp packaging/systemd/*.service "$STAGE/systemd/"
cp packaging/env/*.env.example "$STAGE/env/"
cp packaging/install.sh      "$STAGE/"; chmod +x "$STAGE/install.sh"
cp packaging/README.md       "$STAGE/"

tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo
echo "Package: $OUT/$NAME.tar.gz"
ls -lh "$OUT/$NAME.tar.gz"
