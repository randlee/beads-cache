#!/usr/bin/env bash
# Creates a scratch bd workspace on a scratch Dolt server for the live probes.
#
# Usage: setup-scratch.sh <env file>
#
# Needs `bd` and `dolt` on PATH. Never touches an existing database: it starts its own
# `dolt sql-server` (own data directory, own Dolt configuration, a free loopback port)
# and runs `bd init --server --external` in a new directory. The workspace carries the
# marker `.beads/sc-bd-live-scratch`, which tests/live.rs requires. Writes SC_BD_LIVE,
# SC_BD_LIVE_DIR and SC_BD_LIVE_ROOT to <env file> (sourceable, and valid as a
# GITHUB_ENV file). Remove everything with teardown-scratch.sh.
set -euo pipefail

envfile="${1:?usage: setup-scratch.sh <env file>}"
root="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/sc-bd-live.XXXXXX")"
touch "$root/.sc-bd-live-root"
mkdir -p "$root/dolt-home" "$root/dolt-data" "$root/ws"

port="$(python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
PY
)"

# The scratch server has its own Dolt configuration, so the machine's is left alone.
export DOLT_ROOT_PATH="$root/dolt-home"
dolt config --global --add user.name "sc-bd-live" >/dev/null
dolt config --global --add user.email "sc-bd-live@example.invalid" >/dev/null
(
  cd "$root/dolt-data"
  nohup dolt sql-server --host 127.0.0.1 --port "$port" >"$root/dolt.log" 2>&1 &
  echo $! >"$root/dolt.pid"
)

for _ in $(seq 1 60); do
  if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then break; fi
  sleep 1
done
(exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null || {
  echo "the scratch dolt server did not start; log:" >&2
  cat "$root/dolt.log" >&2
  exit 1
}

cd "$root/ws"
git init -q .
git config user.name "sc-bd-live"
git config user.email "sc-bd-live@example.invalid"
BD_NON_INTERACTIVE=1 bd init --server --external --server-host 127.0.0.1 --server-port "$port" \
  --server-user root --prefix live --non-interactive >"$root/bd-init.log" 2>&1 || {
  cat "$root/bd-init.log" >&2
  exit 1
}
touch "$root/ws/.beads/sc-bd-live-scratch"

{
  echo "SC_BD_LIVE=1"
  echo "SC_BD_LIVE_DIR=$root/ws"
  echo "SC_BD_LIVE_ROOT=$root"
} >>"$envfile"
echo "scratch workspace: $root/ws (dolt on 127.0.0.1:$port)"
