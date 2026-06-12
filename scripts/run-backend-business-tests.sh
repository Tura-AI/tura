#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

exec sh "$REPO_ROOT/xtask/scripts/run-backend-business-tests.sh" "$@"
