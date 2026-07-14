# Docker harness for the command interceptor e2e tests.
#
# The image only carries the Rust toolchain plus the system libraries the
# tools crate needs to build (OpenSSL for reqwest). The repository is bind
# mounted at run time (see scripts/run_interceptor_e2e_docker.*), so this image
# stays source-agnostic and cacheable. Real Linux `bash` and `zsh` shells are
# present so the e2e tests can drive both shell surfaces through `command_run`.
FROM rust:1-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
        zsh \
    && rm -rf /var/lib/apt/lists/*

# Keep build artifacts off the bind-mounted host tree.
ENV CARGO_TARGET_DIR=/tmp/target

WORKDIR /work
