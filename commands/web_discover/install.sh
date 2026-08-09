#!/usr/bin/env sh
# Thin shim — all logic lives in install.py.
exec python3 "$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)/scripts/install.py" command web_discover "$@"
