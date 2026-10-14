#!/usr/bin/env bash
# Serves web/ at http://localhost:${PORT:-8080}. WebAssembly can't load from file://, so open the
# app through this (or any) web server. Run scripts/build-web.sh first.
set -euo pipefail
cd "$(dirname "$0")/../web"
exec python3 -m http.server "${PORT:-8080}" --bind 127.0.0.1
