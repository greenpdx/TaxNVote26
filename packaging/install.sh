#!/usr/bin/env bash
# Tax N Vote installer. Installs both build variants (full + demo) as systemd
# services. Run as root on the target host:
#
#   sudo ./install.sh            # install / upgrade
#   sudo ./install.sh --uninstall
#
# Layout it creates:
#   /opt/tnv/{full,demo}/{tnv-server,static}   binaries + SPA
#   /opt/tnv/data/budauth.csv                  shared dataset
#   /etc/tnv/{full,demo}.env                   config (not overwritten on upgrade)
#   /var/lib/tnv/                              writable state (sqlite, etc.)
#   /etc/systemd/system/tnv-{full,demo}.service
set -euo pipefail

PREFIX="${PREFIX:-/opt/tnv}"
ETC=/etc/tnv
VARLIB=/var/lib/tnv
SRC="$(cd "$(dirname "$0")" && pwd)"
SERVICES=(tnv-full tnv-demo)

require_root() { [ "$(id -u)" = 0 ] || { echo "Run as root (sudo)." >&2; exit 1; }; }

uninstall() {
  require_root
  for s in "${SERVICES[@]}"; do
    systemctl disable --now "$s" 2>/dev/null || true
    rm -f "/etc/systemd/system/$s.service"
  done
  systemctl daemon-reload
  rm -rf "$PREFIX"
  echo "Removed $PREFIX and the systemd services."
  echo "Left in place (remove by hand if you want them gone): $ETC, $VARLIB, the 'tnv' user."
}

install_pkg() {
  require_root

  # 1. Unprivileged service account.
  id tnv >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin tnv

  # 2. Program files (replaced on upgrade).
  install -d "$PREFIX"
  rm -rf "$PREFIX/full" "$PREFIX/demo" "$PREFIX/data"
  cp -r "$SRC/full" "$SRC/demo" "$SRC/data" "$PREFIX/"
  chown -R tnv:tnv "$PREFIX"

  # 3. Writable state.
  install -d -o tnv -g tnv "$VARLIB"

  # 4. Config — never clobber an existing edited env file.
  install -d "$ETC"
  for v in full demo; do
    if [ -f "$ETC/$v.env" ]; then
      echo "kept existing $ETC/$v.env"
    else
      cp "$SRC/env/$v.env.example" "$ETC/$v.env"
      chown root:tnv "$ETC/$v.env"; chmod 640 "$ETC/$v.env"
      echo "created $ETC/$v.env (edit it before starting)"
    fi
  done

  # 5. systemd units.
  cp "$SRC/systemd/"*.service /etc/systemd/system/
  systemctl daemon-reload

  cat <<EOF

Installed to $PREFIX.

Next steps:
  1. Generate a signing key:           openssl rand -hex 32
  2. Edit config (set JWT_SECRET, DATABASE_URL, FISCAL_YEAR):
       $ETC/full.env     (permanent site, :3000)
       $ETC/demo.env     (conference build, :3001 — SQLite by default)
  3. Create an admin login (example, demo):
       sudo -u tnv bash -c 'set -a; . $ETC/demo.env; cd $PREFIX/demo; \\
         ./tnv-server admin create admin@example.com admin <password>'
  4. Start the service(s):
       systemctl enable --now tnv-full     # http://<host>:3000
       systemctl enable --now tnv-demo     # http://<host>:3001

Run only the one you need — they are independent. TLS/reverse proxy is separate.
EOF
}

case "${1:-}" in
  --uninstall|-u) uninstall ;;
  ""|--install)   install_pkg ;;
  *) echo "usage: $0 [--install|--uninstall]" >&2; exit 1 ;;
esac
