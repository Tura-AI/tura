#!/usr/bin/env bash
# Build the interceptor e2e harness image and run the command interceptor
# end-to-end tests inside a Linux container, where a real `bash` executes the
# commands. The interceptor must block dangerous commands before they run.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="tura-interceptor-e2e"
DOCKER_DIR="$ROOT/crates/tools/tests/docker"

# On Git Bash / MSYS for Windows, disable POSIX->Windows path rewriting so the
# container-side `/work` paths are passed through verbatim.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL="*"

docker build -t "$IMAGE" -f "$DOCKER_DIR/Dockerfile" "$DOCKER_DIR"

docker run --rm \
    -v "$ROOT":/work \
    -w /work \
    "$IMAGE" \
    cargo test -p code-tools --test command_interceptor_e2e -- --nocapture
