#!/usr/bin/env bash
# Build the UI and the backend, then serve everything from the Rust binary.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> building UI"
if [ ! -d web/node_modules ]; then
  (cd web && npm install)
fi
(cd web && npm run build)

echo "==> building backend (release)"
(cd backend && cargo build --release)

echo "==> serving on http://127.0.0.1:${PORT:-8787}"
exec ./backend/target/release/re-search-accessory-backend.exe 2>/dev/null \
  || exec ./backend/target/release/re-search-accessory-backend
