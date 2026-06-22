# Tax N Vote — install package

A self-contained bundle of both build variants (no build tools needed on the
target):

| Variant | Sign-in | Port | Use |
|---------|---------|------|-----|
| `full`  | Email + password | 3000 | Permanent public site |
| `demo`  | Name + 4-digit PIN | 3001 | Conference build (admin still email login) |

## Requirements

- **Linux, x86_64, glibc ≥ 2.36** (built on Debian 12 / bookworm). Check the
  target with `uname -m` and `ldd --version`. On an older/incompatible libc,
  build from source or use the Docker image instead.
- `systemd`, `ca-certificates` (outbound TLS for SMTP / Postgres).
- A database: Postgres for the permanent site, or SQLite (bundled, no server).

## Install

```sh
tar xzf tnv-*-linux-x86_64.tar.gz
cd tnv-*/
sudo ./install.sh
```

This installs to `/opt/tnv`, drops config in `/etc/tnv/{full,demo}.env`, and
creates the `tnv-full` / `tnv-demo` systemd services. Then:

1. `openssl rand -hex 32` → put it in `JWT_SECRET`.
2. Edit `/etc/tnv/full.env` and/or `/etc/tnv/demo.env` (set `JWT_SECRET`,
   `DATABASE_URL`, `FISCAL_YEAR`).
3. Create an admin login (works for both variants), e.g. for demo:
   ```sh
   sudo -u tnv bash -c 'set -a; . /etc/tnv/demo.env; cd /opt/tnv/demo; \
     ./tnv-server admin create admin@example.com admin <password>'
   ```
4. Start whichever you need:
   ```sh
   sudo systemctl enable --now tnv-full     # http://<host>:3000
   sudo systemctl enable --now tnv-demo     # http://<host>:3001
   ```

The two services are independent — run one or both. Logs: `journalctl -u tnv-full -f`.

## Notes

- **`FISCAL_YEAR` is baked into each SPA** at build time (this package = 2027).
  The server's `FISCAL_YEAR` must match it; to change the year, rebuild the
  package.
- TLS / reverse proxy are out of scope — put Caddy/nginx in front and set
  `TRUSTED_PROXY=true` once it forwards `X-Forwarded-For`.
- Upgrade: re-run `sudo ./install.sh` from a newer package — it replaces the
  binaries/SPA but keeps your `/etc/tnv/*.env`.
- Uninstall: `sudo ./install.sh --uninstall`.
