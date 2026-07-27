#!/bin/sh
set -e

cargo build -p selector4nix "$@"
exec cargo run -p selector4nix-system-test-nar-info-querying -- \
  --selector4nix ./target/debug/selector4nix
