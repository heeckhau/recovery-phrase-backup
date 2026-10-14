#!/usr/bin/env bash
# The reference release build: scripts/build.py inside one pinned Linux
# x86-64 container, so anyone with Docker gets the same bytes as CI.
#
#   scripts/build-reproducible.sh                        # -> dist/ (release file, demo, site)
#   scripts/build-reproducible.sh --verify-reproducible  # build twice, compare
#
# Why a container: the wasm is byte-identical across builds on one platform,
# but not across platforms: Cargo's crate hashes take in the host (through
# proc-macro crates, which are compiled for it), and those hashes order the
# functions and data in the output. Fixing the platform fixes the output.
# On Apple Silicon this runs under emulation: slower, same result.
set -euo pipefail

IMAGE="rust:1.99.0-bookworm@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6"

cd "$(dirname "$0")/.."
mkdir -p dist

docker run --rm --platform linux/amd64 \
  -v "$PWD":/src:ro -v "$PWD/dist":/out \
  "$IMAGE" bash -c '
    set -euo pipefail
    mkdir /work
    (cd /src && tar --exclude=./target --exclude=./dist --exclude=./web/pkg --exclude=./.git -cf - .) | tar -xf - -C /work
    cd /work
    python3 -I scripts/build.py "$@"
    cp -r dist/. /out/
  ' build "$@"

files=$(find dist -type f -name '*.html' | sort)
if command -v sha256sum >/dev/null; then sha256sum $files; else shasum -a 256 $files; fi
