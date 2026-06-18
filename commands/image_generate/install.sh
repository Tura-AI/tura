#!/usr/bin/env sh
set -eu

while [ "$#" -gt 0 ]; do
  case "$1" in
    --check-only|--offline) ;;
    -h|--help)
      echo "Usage: commands/image_generate/install.sh [--check-only] [--offline]"
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

echo "image_generate dependencies: ok (Rust-only command)"
