#!/usr/bin/env bash
# One-command demo: tests, policy demo, injection demo, wasm build.
set -euo pipefail
cd "$(dirname "$0")"

echo "== tests =="
cargo test

echo
echo "== policy demo =="
cargo run --quiet --example agent_demo

echo
echo "== prompt-injection demo =="
cargo run --quiet --example agent_injection

echo
echo "== wasm =="
cargo build --release --target wasm32-unknown-unknown
ls -l "${CARGO_TARGET_DIR:-target}"/wasm32-unknown-unknown/release/latchguard.wasm 2>/dev/null \
  || ls -l "${CARGO_TARGET_DIR:-target}"/wasm32-unknown-unknown/release/agent_guard.wasm
