#!/usr/bin/env bash
# Development mode: Rust backend on :8787 + Vite dev server on :5173.
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -d web/node_modules ]; then
  (cd web && npm install)
fi

cleanup() {
  if [ -n "${BACKEND_PID:-}" ]; then
    kill "$BACKEND_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "==> backend (port ${PORT:-8787})"
(cd backend && cargo run) &
BACKEND_PID=$!

sleep 2
echo "==> UI (port 5173) — mở http://127.0.0.1:5173"
cd web && npm run dev
