#!/usr/bin/env bash
# Clone event-contract sibling repos (skip existing) and build release binaries.
# Does not start Reth, the node, or the app. Run start-events.sh after this.
set -euo pipefail

if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi
if [[ -s "${HOME}/.nvm/nvm.sh" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.nvm/nvm.sh"
fi
export PATH="${HOME}/.foundry/bin:${HOME}/.cargo/bin:${PATH}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LABS="$(cd "$APP_ROOT/.." && pwd)"

REPOS=(
  lightpool-node
  lightpool-crypto
  lightpool-sdk-rust
  lightpool-clob-indexer
  lightpool-bridge
  lightpool-bot
  lightpool-tutorials
)

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing command: $1" >&2
    echo "run ./scripts/download-toolchain.sh first" >&2
    exit 1
  }
}

clone_repo() {
  local name="$1"
  local dest="$LABS/$name"
  if [[ -d "$dest/.git" ]]; then
    echo "skip  $name (already cloned)"
    return
  fi
  if [[ -e "$dest" ]]; then
    echo "path exists but is not a git checkout: $dest" >&2
    exit 1
  fi
  echo "clone $name"
  git clone "git@github.com:lightpool-labs/${name}.git" "$dest"
}

echo "LABS=$LABS"
echo "APP=$APP_ROOT"

need_cmd git
need_cmd curl
need_cmd rustc
need_cmd cargo
need_cmd node
need_cmd npm
need_cmd forge
need_cmd python3

echo "rustc $(rustc --version)"
echo "cargo $(cargo --version)"
echo "node $(node --version)"
echo "npm $(npm --version)"
echo "forge $(forge --version)"
echo "python3 $(python3 --version)"

for name in "${REPOS[@]}"; do
  clone_repo "$name"
done

echo "download reth"
"$LABS/lightpool-node/tools/reth/download.sh"

echo "build lightpool-node"
(cd "$LABS/lightpool-node" && cargo build --release)

echo "build lightpool-clob-indexer"
(cd "$LABS/lightpool-clob-indexer" && cargo build --release)

echo "build lightpool-bridge"
(cd "$LABS/lightpool-bridge" && cargo build --release --bin lightpool-bridge)

echo "build liquidity-maker"
(cd "$APP_ROOT/liquidity-maker" && cargo build --release --bin liquidity-maker)

echo "build event-contract-app backend"
(cd "$APP_ROOT/backend" && cargo build --release)

if [[ ! -d "$APP_ROOT/frontend/node_modules" ]]; then
  echo "npm install (frontend)"
  (cd "$APP_ROOT/frontend" && npm install)
else
  echo "skip  frontend node_modules"
fi

CONTRACTS="$LABS/lightpool-bridge/contracts"
if [[ ! -f "$CONTRACTS/lib/forge-std/src/Test.sol" ]]; then
  echo "forge install foundry-rs/forge-std"
  (cd "$CONTRACTS" && forge install foundry-rs/forge-std --no-commit)
else
  echo "skip  forge-std"
fi

echo "ready"
echo "  reth          $LABS/lightpool-node/tools/reth/bin/reth"
echo "  lightpool     $LABS/lightpool-node/bin/lightpool"
echo "  indexer       $LABS/lightpool-clob-indexer/target/release/lightpool-clob-indexer"
echo "  bridge        $LABS/lightpool-bridge/target/release/lightpool-bridge"
echo "  maker         $APP_ROOT/liquidity-maker/target/release/liquidity-maker"
echo "  backend       $APP_ROOT/backend/target/release/event-contract-backend"
echo "next: ./scripts/start-events.sh start"
