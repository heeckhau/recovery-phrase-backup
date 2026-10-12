#!/usr/bin/env bash
# Runs the cross-tool recovery suite in the pinned nix shell. Pass
# --check-only to verify without rewriting slip39/tests/interop/*.json.
set -euo pipefail
cd "$(dirname "$0")"
exec nix-shell shell.nix --run "python3 -I run.py $*"
