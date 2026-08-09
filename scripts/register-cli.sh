#!/usr/bin/env sh
# Thin shim — all logic lives in install.py.
exec python3 "$(dirname -- "$0")/install.py" register-cli "$@"
