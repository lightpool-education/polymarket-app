#!/usr/bin/env bash
# Start the local event-contract stack (with bridge).
# 1. Reth
# 2. LightPool node + clob-indexer (via lightpool-tutorials/scripts/run-venue.sh)
# 3. USDT on Reth + LP USDT + bridge route
# 4. Fund maker deposit on LightPool
# 5. event-contract-app backend + frontend
# 6. liquidity-maker — bootstrap the hottest Polymarket events onto LightPool
#
# Usage:
#   ./scripts/start-events.sh start
#   ./scripts/start-events.sh start <polymarket-slug>
#   ./scripts/start-events.sh stop
#   ./scripts/start-events.sh clean
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

MAKER_KEY="${MAKER_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
MAKER_ADDR="${MAKER_ADDR:-0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266}"
USER_KEY="${USER_KEY:-0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d}"
USER_ADDR="${USER_ADDR:-0x70997970C51812dc3A010C7d01b50e0d17dc79C8}"

RETH_RPC="${RETH_RPC:-http://127.0.0.1:8545}"
LP_RPC="${LP_RPC:-http://127.0.0.1:26300}"
CLOB_HTTP="${LIGHTPOOL_CLOB_INDEX_HTTP:-http://127.0.0.1:3002}"
CLOB_WS="${LIGHTPOOL_CLOB_INDEX_WS:-ws://127.0.0.1:3002}"
APP_API="${APP_API:-http://127.0.0.1:3001/api}"

# Whole USDT (6 decimals on chain). Must cover HOT_EVENTS * MAX_MARKETS * (MINT_AMOUNT / 1e6).
# 9 events * 4 markets * 1_000_000 USDT = 36_000_000, plus headroom.
MAKER_DEPOSIT_WHOLE="${MAKER_DEPOSIT_WHOLE:-100000000}"
USER_TOKEN_WHOLE="${USER_TOKEN_WHOLE:-10000}"
HOT_EVENTS="${HOT_EVENTS:-9}"
MAX_MARKETS="${MAX_MARKETS:-4}"
# Raw units (6 decimals). Default 1e12 = 1_000_000 USDT per market.
MINT_AMOUNT="${MINT_AMOUNT:-1000000000000}"

resolve_labs() {
  if [[ -n "${LABS:-}" ]]; then
    printf '%s\n' "$LABS"
    return
  fi
  local candidate
  candidate="$(cd "$APP_ROOT/.." && pwd)"
  if [[ -d "$candidate/lightpool-node" || -d "$candidate/lightpool" ]]; then
    printf '%s\n' "$candidate"
    return
  fi
  printf '%s\n' "$(pwd)/lightpool-labs"
}

LABS="$(resolve_labs)"
DATA="${DATA:-$LABS/data/event-contract}"
export LABS DATA
LOG_DIR="$DATA/logs"
PID_DIR="$DATA/pids"
WALLET_PATH="$DATA/wallet.json"
STATE="$DATA/eth-bridge.json"
BRIDGE_CONFIG="$DATA/bridge-config.json"
SLUG_FILE="$DATA/polymarket-slug.txt"
RETH_DATADIR="$DATA/reth"
FORGE_DIR="$DATA/forge"
APP_DIR="${APP_DIR:-$APP_ROOT}"
MAKER_DIR="${MAKER_DIR:-$APP_DIR/liquidity-maker}"
CONTRACTS="$LABS/lightpool-bridge/contracts"
RETH_BIN="${RETH_BIN:-$LABS/lightpool-node/tools/reth/bin/reth}"
BRIDGE_BIN="${BRIDGE_BIN:-$LABS/lightpool-bridge/target/release/lightpool-bridge}"
VENUE="${VENUE:-$LABS/lightpool-tutorials/scripts/run-venue.sh}"

if [[ -z "${LIGHTPOOL_BIN:-}" ]]; then
  if [[ -x "$LABS/lightpool/target/release/lightpool" ]]; then
    LIGHTPOOL_BIN="$LABS/lightpool/target/release/lightpool"
  else
    LIGHTPOOL_BIN="$LABS/lightpool-node/bin/lightpool"
  fi
fi

export PATH="${HOME}/.foundry/bin:${PATH:-}"

usage() {
  cat <<EOF
Usage: $(basename "$0") start [polymarket-slug]
       $(basename "$0") stop|clean|status|help

Start local Reth, LightPool, indexer, USDT bridge, event-contract app, and
liquidity-maker. With no slug, the top \$HOT_EVENTS Polymarket events by 24-hour
volume are bootstrapped onto LightPool. An optional slug pins one Gamma event.

  start          Bootstrap the hottest \$HOT_EVENTS events (default 9)
  start <slug>   Bootstrap one Polymarket Gamma event slug
  stop           Stop bot, app, bridge, Reth, node, indexer. Keep data.
  clean          Delete \$DATA only. Does not stop. Run stop first if needed.
  status         Show PIDs

Env:
  LABS / DATA / HOT_EVENTS / MAX_MARKETS / MINT_AMOUNT / HTTPS_PROXY
  MAKER_DEPOSIT_WHOLE  maker USDT deposited via bridge (default 100000000)
  USER_TOKEN_WHOLE     ERC20 USDT minted to demo user on Reth (default 10000)

Runtime state under \$DATA ($DATA).
Maker (Anvil #0): $MAKER_ADDR
User  (Anvil #1): $USER_ADDR
EOF
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing command: $1" >&2
    exit 1
  }
}

mkdirs() {
  mkdir -p "$LOG_DIR" "$PID_DIR" "$RETH_DATADIR" "$FORGE_DIR"
}

prepare_forge_home() {
  mkdir -p "$FORGE_DIR"
  cat >"$FORGE_DIR/foundry.toml" <<EOF
[profile.default]
src = "${CONTRACTS}/src"
script = "${CONTRACTS}/script"
out = "${FORGE_DIR}/out"
libs = ["${CONTRACTS}/lib"]
cache_path = "${FORGE_DIR}/cache"
broadcast = "${FORGE_DIR}/broadcast"
solc_version = "0.8.24"
optimizer = true
optimizer_runs = 200
remappings = ["forge-std/=${CONTRACTS}/lib/forge-std/src/"]
EOF
}

is_running() {
  local name="$1"
  local pid_file="$PID_DIR/$name.pid"
  [[ -f "$pid_file" ]] || return 1
  kill -0 "$(cat "$pid_file")" 2>/dev/null
}

stop_pid() {
  local name="$1"
  local pid_file="$PID_DIR/$name.pid"
  [[ -f "$pid_file" ]] || return 0
  local pid
  pid="$(cat "$pid_file")"
  if kill -0 "$pid" 2>/dev/null; then
    echo "stop  $name (pid $pid)"
    kill -- -"$pid" 2>/dev/null || kill "$pid" 2>/dev/null || true
    local _
    for _ in $(seq 1 20); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.2
    done
    kill -9 -- -"$pid" 2>/dev/null || kill -9 "$pid" 2>/dev/null || true
  fi
  rm -f "$pid_file"
}

stop_port() {
  local port="$1"
  local name="$2"
  local pids pid
  pids="$(ss -lptnH "sport = :$port" 2>/dev/null | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u || true)"
  [[ -n "$pids" ]] || return 0
  for pid in $pids; do
    echo "stop  $name (pid $pid port $port)"
    kill "$pid" 2>/dev/null || true
  done
  sleep 0.5
  for pid in $pids; do
    kill -0 "$pid" 2>/dev/null || continue
    kill -9 "$pid" 2>/dev/null || true
  done
}

wait_http() {
  local url="$1"
  local name="$2"
  local i body
  echo "wait  $name $url"
  for i in $(seq 1 90); do
    body="$(curl -fsS "$url" 2>/dev/null || true)"
    if [[ -n "$body" ]]; then
      echo "ok    $name"
      return 0
    fi
    sleep 1
  done
  echo "$name not ready: $url" >&2
  exit 1
}

wait_reth() {
  local i
  echo "wait  Reth $RETH_RPC"
  for i in $(seq 1 60); do
    if cast chain-id --rpc-url "$RETH_RPC" >/dev/null 2>&1; then
      echo "ok    Reth chain-id=$(cast chain-id --rpc-url "$RETH_RPC")"
      return 0
    fi
    sleep 1
  done
  echo "Reth not ready: $RETH_RPC" >&2
  exit 1
}

start_reth() {
  if is_running reth; then
    echo "skip  Reth (already running, datadir $RETH_DATADIR)"
    return
  fi
  if cast chain-id --rpc-url "$RETH_RPC" >/dev/null 2>&1; then
    echo "stop  Reth on :8545 so chain data stays in $RETH_DATADIR"
    stop_port 8545 reth
  fi
  if [[ ! -x "$RETH_BIN" ]]; then
    echo "reth not found: $RETH_BIN" >&2
    exit 1
  fi
  echo "start Reth datadir $RETH_DATADIR"
  setsid "$RETH_BIN" node \
    --dev \
    --dev.block-time 1s \
    --datadir "$RETH_DATADIR" \
    --http \
    --http.addr 127.0.0.1 \
    --http.port 8545 \
    --http.api eth,net,web3,txpool,debug,trace \
    --http.corsdomain '*' \
    --ws \
    --ws.addr 127.0.0.1 \
    --ws.port 8546 \
    --ws.api eth,net,web3,txpool,debug,trace \
    --ws.origins '*' \
    --authrpc.addr 127.0.0.1 \
    --authrpc.port 8551 \
    --engine.persistence-threshold 0 \
    --engine.memory-block-buffer-target 0 \
    >"$LOG_DIR/reth.log" 2>&1 < /dev/null &
  echo $! >"$PID_DIR/reth.pid"
  wait_reth
}

lp() {
  "$LIGHTPOOL_BIN" --rpc-url "$LP_RPC" --wallet-path "$WALLET_PATH" "$@"
}

raw_amount() {
  python3 -c "print(int('$1') * 1_000_000)"
}

parse_labeled() {
  python3 - "$1" "$2" "$3" <<'PY'
import re, sys
text = open(sys.argv[1], errors="replace").read()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
label, prefix = sys.argv[2], sys.argv[3]
match = re.search(re.escape(label) + r"\s*:?\s*(" + prefix + r"[0-9a-fA-F]{14})", text, re.I)
if not match:
    sys.exit(1)
print(match.group(1))
PY
}

require_lp_ok() {
  local log="$1"
  if grep -Eiq 'initialization failed|Failed to submit transaction|Failed to connect to node|Wallet not found' "$log"; then
    echo "lightpool command failed; see $log" >&2
    exit 1
  fi
}

validator_stake() {
  local body stake
  body="$(curl -fsS -m 10 -X POST "$LP_RPC" -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getCommitteeInfo","params":[]}' 2>/dev/null || true)"
  stake="$(python3 -c 'import json,sys
raw=sys.stdin.read().strip()
if not raw:
    print("100"); raise SystemExit
try:
    members=(json.loads(raw).get("result") or {}).get("members") or []
except Exception:
    print("100"); raise SystemExit
want="'"$MAKER_ADDR"'".lower()
for member in members:
    if str(member.get("owner") or "").lower()==want and member.get("stake") is not None:
        print(member["stake"]); raise SystemExit
if members and members[0].get("stake") is not None:
    print(members[0]["stake"]); raise SystemExit
print("100")' <<<"$body")"
  printf '%s\n' "$stake"
}

deploy_usdt_and_bridge() {
  local log="$LOG_DIR/forge-usdt.log" stake
  if [[ ! -f "$CONTRACTS/lib/forge-std/src/Test.sol" ]]; then
    echo "forge-std missing under $CONTRACTS/lib. Run: forge install foundry-rs/forge-std" >&2
    echo "(from $CONTRACTS)" >&2
    exit 1
  fi
  stake="$(validator_stake)"
  echo "deploy USDT + EVM Bridge (validator stake $stake)"
  (
    cd "$CONTRACTS"
    VALIDATOR_ETH="$MAKER_ADDR" \
    VALIDATOR_STAKE="$stake" \
    USER_ETH="$USER_ADDR" \
    forge script script/DeployLocal.s.sol:DeployLocal \
      --rpc-url "$RETH_RPC" \
      --broadcast \
      --private-key "$MAKER_KEY" \
      -vv \
      --root "$CONTRACTS" \
      --config-path "$FORGE_DIR/foundry.toml" \
      --cache-path "$FORGE_DIR/cache" \
      -o "$FORGE_DIR/out"
  ) | tee "$log"
  USDT_EVM="$(python3 -c 'import re,sys; t=open(sys.argv[1],errors="replace").read(); m=re.search(r"USDT\s+(0x[0-9a-fA-F]{40})", t); print(m.group(1) if m else "")' "$log")"
  EVM_BRIDGE="$(python3 -c 'import re,sys; t=open(sys.argv[1],errors="replace").read(); m=re.search(r"BRIDGE\s+(0x[0-9a-fA-F]{40})", t); print(m.group(1) if m else "")' "$log")"
  if [[ -z "$USDT_EVM" || -z "$EVM_BRIDGE" ]]; then
    echo "failed to parse USDT / BRIDGE from $log" >&2
    exit 1
  fi
  echo "ok    USDT $USDT_EVM"
  echo "ok    EVM Bridge $EVM_BRIDGE"
}

mint_erc20() {
  local token="$1" to="$2" whole="$3"
  local raw
  raw="$(raw_amount "$whole")"
  cast send "$token" "mint(address,uint256)" "$to" "$raw" \
    --rpc-url "$RETH_RPC" --private-key "$MAKER_KEY" >/dev/null
}

fund_eth() {
  local to="$1" amount="$2"
  cast send "$to" --value "$amount" --rpc-url "$RETH_RPC" --private-key "$MAKER_KEY" >/dev/null \
    || echo "warning: could not send $amount ETH to $to"
}

init_lp_usdt() {
  local log lp inbound
  log="$LOG_DIR/init-USDT.log"
  echo "create LightPool USDT (init-bridge)"
  lp init-bridge \
    --foreign-chain-id "$EVM_CHAIN_ID" \
    --foreign-token "$USDT_EVM" \
    --name "Tether USD" \
    --symbol USDT >"$log" 2>&1 || true
  cat "$log"
  require_lp_ok "$log"
  lp="$(parse_labeled "$log" "LP token" "0x02" || true)"
  inbound="$(parse_labeled "$log" "Inbound bridge contract" "0x06" || true)"
  if [[ -z "$lp" || -z "$inbound" ]]; then
    echo "failed to parse LP USDT; see $log" >&2
    exit 1
  fi
  echo "ok    USDT lp=$lp inbound=$inbound"
  LP_USDT="$lp"
  IN_USDT="$inbound"
}

save_state() {
  python3 - "$STATE" "$EVM_CHAIN_ID" "$EVM_BRIDGE" \
    "$MAKER_ADDR" "$USER_ADDR" \
    "$USDT_EVM" "$LP_USDT" "$IN_USDT" <<'PY'
import json, sys
(
    state_path, chain_id, evm_bridge, maker, user,
    usdt_evm, lp_usdt, in_usdt,
) = sys.argv[1:]
state = {
    "evm_chain_id": int(chain_id),
    "evm_bridge": evm_bridge,
    "maker": maker,
    "user": user,
    "tokens": [
        {
            "symbol": "USDT",
            "name": "Tether USD",
            "evm": usdt_evm,
            "lp": lp_usdt,
            "inbound": in_usdt,
        }
    ],
}
with open(state_path, "w", encoding="utf-8") as handle:
    json.dump(state, handle, indent=2)
    handle.write("\n")
print(f"wrote {state_path}")
PY
}

setup_usdt_and_bridge_map() {
  if [[ -f "$STATE" ]]; then
    echo "skip  USDT / bridge setup ($STATE already exists)"
    return
  fi
  EVM_CHAIN_ID="$(cast chain-id --rpc-url "$RETH_RPC")"
  prepare_forge_home
  echo "=== create USDT on Reth + LightPool bridge map ==="
  fund_eth "$USER_ADDR" 10ether
  deploy_usdt_and_bridge
  mint_erc20 "$USDT_EVM" "$MAKER_ADDR" "$MAKER_DEPOSIT_WHOLE"
  mint_erc20 "$USDT_EVM" "$USER_ADDR" "$USER_TOKEN_WHOLE"
  init_lp_usdt
  save_state
}

write_bridge_config() {
  python3 - "$STATE" "$BRIDGE_CONFIG" "$WALLET_PATH" "$LP_RPC" "$RETH_RPC" <<'PY'
import json, sys
state_path, config_path, wallet, lp_rpc, reth_rpc = sys.argv[1:]
state = json.load(open(state_path, encoding="utf-8"))
routes = []
for token in state["tokens"]:
    routes.append({
        "id": "reth-" + token["symbol"].lower(),
        "enabled": True,
        "local_inbound": {
            "bridge_contract": token["inbound"],
            "lp_token": token["lp"],
        },
        "foreign": {
            "kind": "evm",
            "rpc_url": reth_rpc,
            "chain_id": state["evm_chain_id"],
            "bridge_address": state["evm_bridge"],
            "token_address": token["evm"],
            "confirmations": 1,
            "start_block": 0,
        },
    })
config = {
    "wallet_path": wallet,
    "lightpool_rpc_url": lp_rpc,
    "cast_bin": "cast",
    "local": {"rpc_url": lp_rpc, "chain_id": 1},
    "routes": routes,
}
with open(config_path, "w", encoding="utf-8") as handle:
    json.dump(config, handle, indent=2)
    handle.write("\n")
print(f"wrote {config_path} ({len(routes)} routes)")
PY
}

start_bridge() {
  if is_running bridge; then
    echo "skip  bridge (already running)"
    return
  fi
  if [[ ! -x "$BRIDGE_BIN" ]]; then
    echo "lightpool-bridge not found: $BRIDGE_BIN" >&2
    exit 1
  fi
  echo "start bridge"
  setsid "$BRIDGE_BIN" --config "$BRIDGE_CONFIG" \
    >"$LOG_DIR/bridge.log" 2>&1 < /dev/null &
  echo $! >"$PID_DIR/bridge.pid"
  sleep 2
  if ! is_running bridge; then
    echo "bridge exited; see $LOG_DIR/bridge.log" >&2
    exit 1
  fi
  echo "ok    bridge"
}

lp_available_raw() {
  local token="$1" log available
  log="$LOG_DIR/balance-check.log"
  lp balance --token-address "$token" --account "$MAKER_ADDR" >"$log" 2>&1 || true
  available="$(python3 -c 'import re,sys
text=open(sys.argv[1], errors="replace").read()
match=re.search(r"Available:\s*([0-9]+(?:\.[0-9]+)?)", text)
if not match:
    print("0"); raise SystemExit
text=match.group(1)
if "." in text:
    whole, frac = text.split(".", 1)
    frac = (frac + "000000")[:6]
    print(int(whole) * 1_000_000 + int(frac))
else:
    print(int(text) * 1_000_000)' "$log")"
  printf '%s\n' "$available"
}

deposit_maker_usdt() {
  echo "=== deposit maker USDT via bridge ==="
  local evm lp_token need have raw i now
  EVM_BRIDGE="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["evm_bridge"])' "$STATE")"
  evm="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["tokens"][0]["evm"])' "$STATE")"
  lp_token="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["tokens"][0]["lp"])' "$STATE")"
  need="$(raw_amount "$MAKER_DEPOSIT_WHOLE")"
  have="$(lp_available_raw "$lp_token")"
  if [[ "$have" -ge "$need" ]]; then
    echo "skip  maker deposit USDT (already credited)"
    return
  fi
  # Top up Reth ERC20 so approve+deposit can cover the gap (first setup may have minted less).
  mint_erc20 "$evm" "$MAKER_ADDR" "$MAKER_DEPOSIT_WHOLE"
  raw="$need"
  echo "deposit maker USDT raw=$raw"
  cast send "$evm" "approve(address,uint256)" "$EVM_BRIDGE" "$raw" \
    --rpc-url "$RETH_RPC" --private-key "$MAKER_KEY" >/dev/null
  cast send "$EVM_BRIDGE" "deposit(address,uint64,address)" "$evm" "$raw" "$MAKER_ADDR" \
    --rpc-url "$RETH_RPC" --private-key "$MAKER_KEY" >/dev/null
  for i in $(seq 1 45); do
    now="$(lp_available_raw "$lp_token")"
    if [[ "$now" -ge "$need" ]]; then
      echo "ok    maker USDT credited"
      return
    fi
    sleep 2
  done
  echo "timed out waiting for maker USDT on LightPool" >&2
  exit 1
}

write_backend_env() {
  local chain_id evm_bridge eth_usdt lp_usdt
  chain_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["evm_chain_id"])' "$STATE")"
  evm_bridge="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["evm_bridge"])' "$STATE")"
  eth_usdt="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["tokens"][0]["evm"])' "$STATE")"
  lp_usdt="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["tokens"][0]["lp"])' "$STATE")"
  cat >"$APP_DIR/backend/.env" <<EOF
HOST=0.0.0.0
PORT=3001
CLOB_INDEX_URL=$CLOB_HTTP
DATABASE_URL=memory
CASH_TOKEN_ADDRESS=$lp_usdt
CASH_TOKEN_SYMBOL=USDT
ETH_USDT=$eth_usdt
BRIDGE=$evm_bridge
EVM_RPC_URL=$RETH_RPC
EVM_CHAIN_ID=$chain_id
JWT_SECRET=dev-jwt-secret-change-me
AGENT_ENCRYPTION_KEY=dev-agent-encryption-key
EOF
  echo "ok    backend .env (CASH_TOKEN_ADDRESS=$lp_usdt)"
}

write_frontend_env() {
  local chain_id
  chain_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["evm_chain_id"])' "$STATE")"
  cat >"$APP_DIR/frontend/.env.local" <<EOF
NEXT_PUBLIC_API_URL=http://127.0.0.1:3001/api
NEXT_PUBLIC_EVM_RPC_URL=$RETH_RPC
NEXT_PUBLIC_EVM_CHAIN_ID=$chain_id
NEXT_PUBLIC_CLOB_INDEX_URL=$CLOB_HTTP
NEXT_PUBLIC_CLOB_INDEX_WS_URL=$CLOB_WS
EOF
  echo "ok    frontend .env.local"
}

ensure_backend_bin() {
  if [[ -n "${BACKEND_BIN:-}" ]]; then
    if [[ ! -x "$BACKEND_BIN" ]]; then
      echo "BACKEND_BIN not executable: $BACKEND_BIN" >&2
      exit 1
    fi
    printf '%s\n' "$BACKEND_BIN"
    return
  fi
  need_cmd cargo
  local bin="$APP_DIR/backend/target/release/event-contract-backend"
  # Always rebuild: PlaceOrderParams must match the running lightpool node (e.g. cloid).
  echo "build event-contract-backend (release)" >&2
  (cd "$APP_DIR/backend" && cargo build --release) >&2
  if [[ ! -x "$bin" ]]; then
    echo "backend binary missing after build: $bin" >&2
    exit 1
  fi
  printf '%s\n' "$bin"
}

ensure_bot_bin() {
  if [[ -n "${BOT_BIN:-}" ]]; then
    if [[ ! -x "$BOT_BIN" ]]; then
      echo "BOT_BIN not executable: $BOT_BIN" >&2
      exit 1
    fi
    printf '%s\n' "$BOT_BIN"
    return
  fi
  need_cmd cargo
  local bin="$MAKER_DIR/target/release/liquidity-maker"
  # Always rebuild: mint/place params must match the running lightpool node (e.g. whitelist).
  echo "build liquidity-maker (release)" >&2
  (cd "$MAKER_DIR" && cargo build --release --bin liquidity-maker) >&2
  if [[ ! -x "$bin" ]]; then
    echo "bot binary missing after build: $bin" >&2
    exit 1
  fi
  printf '%s\n' "$bin"
}

start_backend() {
  local bin
  bin="$(ensure_backend_bin)"
  if is_running backend; then
    echo "restart backend (use latest env / binary)"
    stop_pid backend
    stop_port 3001 backend
  fi
  echo "start backend"
  (
    cd "$APP_DIR/backend"
    setsid "$bin" >"$LOG_DIR/backend.log" 2>&1 < /dev/null &
    echo $! >"$PID_DIR/backend.pid"
  )
}

start_frontend() {
  if is_running frontend; then
    echo "skip  frontend (already running)"
    return
  fi
  if [[ ! -d "$APP_DIR/frontend/node_modules" ]]; then
    echo "npm install (frontend)"
    (cd "$APP_DIR/frontend" && npm install)
  fi
  echo "start frontend"
  (
    cd "$APP_DIR/frontend"
    setsid npm run dev >"$LOG_DIR/frontend.log" 2>&1 < /dev/null &
    echo $! >"$PID_DIR/frontend.pid"
  )
}

gamma_proxy() {
  if [[ -n "${HTTPS_PROXY:-}" ]]; then
    printf '%s\n' "$HTTPS_PROXY"
  elif [[ -n "${https_proxy:-}" ]]; then
    printf '%s\n' "$https_proxy"
  elif [[ -n "${HTTP_PROXY:-}" ]]; then
    printf '%s\n' "$HTTP_PROXY"
  elif [[ -n "${http_proxy:-}" ]]; then
    printf '%s\n' "$http_proxy"
  else
    printf '%s\n' "http://127.0.0.1:8118"
  fi
}

fetch_hottest_event_slugs() {
  local limit="$1"
  local proxy url body
  proxy="$(gamma_proxy)"
  local fetch_limit=$((limit * 5))
  url="https://gamma-api.polymarket.com/events?active=true&closed=false&archived=false&order=volume24hr&ascending=false&limit=${fetch_limit}"
  echo "fetch hottest $limit Polymarket events (volume24hr) via $proxy" >&2
  body="$(curl -fsS -x "$proxy" "$url")" || {
    echo "Polymarket Gamma request failed. Set HTTPS_PROXY and retry." >&2
    exit 1
  }
  printf '%s' "$body" | python3 -c '
import json, sys
limit = int(sys.argv[1])
payload = json.load(sys.stdin)
events = payload.get("data") if isinstance(payload, dict) else payload
if not isinstance(events, list):
    events = []
from datetime import datetime, timedelta, timezone

def deadline_open(raw):
    if raw is None or not str(raw).strip():
        return False
    text = str(raw).strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    if "." in text:
        head, rest = text.split(".", 1)
        if "+" in rest:
            text = head + "+" + rest.split("+", 1)[1]
        else:
            text = head + "+00:00"
    try:
        dt = datetime.fromisoformat(text)
    except ValueError:
        return False
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt > datetime.now(timezone.utc) + timedelta(days=7)

def has_order_book(market):
    if market.get("closed"):
        return False
    if market.get("enableOrderBook") is False or market.get("acceptingOrders") is False:
        return False
    return market.get("bestBid") is not None or market.get("bestAsk") is not None

count = 0
for event in events:
    if event.get("closed") or event.get("archived"):
        continue
    markets = event.get("markets") or []
    if markets:
        if not any(deadline_open(market.get("endDate")) for market in markets):
            continue
    elif not deadline_open(event.get("endDate")):
        continue
    if markets and not any(has_order_book(market) for market in markets):
        continue
    slug = str(event.get("slug") or "").strip()
    if not slug:
        continue
    print(slug)
    count += 1
    if count >= limit:
        break
if count == 0:
    raise SystemExit("no active Polymarket events in Gamma response")
' "$limit"
}

start_bot() {
  local slug="${1:-}"
  local bin lp_usdt
  local -a slug_args=()
  bin="$(ensure_bot_bin)"
  lp_usdt="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["tokens"][0]["lp"])' "$STATE")"
  if [[ -n "$slug" ]]; then
    printf '%s\n' "$slug" >"$SLUG_FILE"
    slug_args=(--polymarket-slug "$slug")
    echo "start liquidity-maker --polymarket-slug $slug (bootstrap max $MAX_MARKETS per event)"
  else
    local hot slug_line
    hot="$(fetch_hottest_event_slugs "$HOT_EVENTS")"
    printf '%s\n' "$hot" >"$SLUG_FILE"
    while IFS= read -r slug_line; do
      [[ -n "$slug_line" ]] || continue
      slug_args+=(--polymarket-slug "$slug_line")
    done <<<"$hot"
    echo "start liquidity-maker hottest $HOT_EVENTS events (max $MAX_MARKETS markets each)"
  fi
  if is_running bot; then
    echo "restart liquidity-maker"
    stop_pid bot
  fi
  (
    cd "$MAKER_DIR"
    export LIGHTPOOL_CLOB_INDEX_HTTP="$CLOB_HTTP"
    export LIGHTPOOL_CLOB_INDEX_WS="$CLOB_WS"
    export LIGHTPOOL_COLLATERAL_TOKEN="$lp_usdt"
    export LIGHTPOOL_PRIVATE_KEY="$MAKER_KEY"
    setsid "$bin" \
      "${slug_args[@]}" \
      --bootstrap-markets \
      --max-markets "$MAX_MARKETS" \
      --mint-amount "$MINT_AMOUNT" \
      >"$LOG_DIR/bot.log" 2>&1 < /dev/null &
    echo $! >"$PID_DIR/bot.pid"
  )
  sleep 2
  if ! is_running bot; then
    echo "liquidity-maker exited early; see $LOG_DIR/bot.log" >&2
    echo "If Polymarket fetch failed, set HTTPS_PROXY and restart: $0 start" >&2
    exit 1
  fi
  echo "ok    liquidity-maker (log $LOG_DIR/bot.log)"
}

print_summary() {
  local slug="${1:-}"
  cat <<EOF

=== event-contract stack ===
LABS   $LABS
DATA   $DATA
Events $(if [[ -n "$slug" ]]; then printf '%s' "$slug"; else printf 'hottest %s (see %s)' "$HOT_EVENTS" "$SLUG_FILE"; fi)
UI     http://127.0.0.1:3000
API    $APP_API
CLOB   $CLOB_HTTP
Reth   $RETH_RPC
Node   $LP_RPC

Import demo user into MetaMask:
  address      $USER_ADDR
  private key  $USER_KEY

Maker / validator / bot (Anvil #0):
  address      $MAKER_ADDR
  private key  $MAKER_KEY

Logs: $LOG_DIR
Stop: $0 stop
Clean: $0 clean

Open the UI and trade the bootstrapped Polymarket events.
If the market list is empty, set HTTPS_PROXY for Polymarket and re-run start.
EOF
  if [[ -f "$STATE" ]]; then
    python3 - "$STATE" "$RETH_RPC" <<'PY'
import json, sys
state = json.load(open(sys.argv[1], encoding="utf-8"))
rpc = sys.argv[2]
print()
print("MetaMask network")
print(f"  RPC URL    {rpc}")
print(f"  Chain ID   {state.get('evm_chain_id')}")
print(f"  Bridge     {state.get('evm_bridge')}")
print("Add ERC20 (6 decimals):")
for token in state.get("tokens") or []:
    print(f"  {token.get('symbol'):<6} {token.get('evm')}")
PY
  fi
}

start_all() {
  local slug="$1"
  need_cmd curl
  need_cmd ss
  need_cmd cast
  need_cmd forge
  need_cmd python3
  need_cmd npm
  if [[ ! -x "$LIGHTPOOL_BIN" ]]; then
    echo "lightpool binary not found: $LIGHTPOOL_BIN" >&2
    exit 1
  fi
  if [[ ! -x "$VENUE" ]]; then
    echo "run-venue.sh not found: $VENUE" >&2
    exit 1
  fi
  mkdirs
  echo "=== start Reth ==="
  start_reth
  echo "=== start LightPool node and indexer ==="
  "$VENUE" start
  setup_usdt_and_bridge_map
  echo "=== map USDT on the bridge ==="
  write_bridge_config
  start_bridge
  deposit_maker_usdt
  write_backend_env
  write_frontend_env
  echo "=== start backend and frontend ==="
  start_backend
  start_frontend
  wait_http "$APP_API/health/health" "backend"
  echo "=== start liquidity-maker ==="
  start_bot "$slug"
  print_summary "$slug"
}

stop_all() {
  need_cmd ss
  echo "=== stop bot ==="
  stop_pid bot
  echo "=== stop backend and frontend ==="
  stop_pid frontend
  stop_pid backend
  stop_port 3000 frontend
  stop_port 3001 backend
  echo "=== stop bridge ==="
  stop_pid bridge
  stop_port 8787 bridge
  echo "=== stop Reth ==="
  stop_pid reth
  stop_port 8545 reth
  echo "=== stop LightPool node and indexer ==="
  if [[ -x "$VENUE" ]]; then
    "$VENUE" stop
  else
    stop_pid indexer
    stop_pid lightpool
  fi
  echo "stopped"
}

clean_all() {
  local name
  for name in bot frontend backend bridge reth lightpool indexer; do
    if is_running "$name"; then
      echo "services still running; run: $0 stop" >&2
      echo "then: $0 clean" >&2
      exit 1
    fi
  done
  rm -rf "$DATA" "$CONTRACTS/broadcast" "$CONTRACTS/cache"
  echo "cleaned $DATA"
}

status_all() {
  echo "LABS=$LABS"
  echo "DATA=$DATA"
  if [[ -f "$SLUG_FILE" ]]; then
    echo "EVENTS:"
    sed 's/^/  /' "$SLUG_FILE"
  fi
  local name
  for name in reth lightpool indexer bridge backend frontend bot; do
    if is_running "$name"; then
      echo "UP   $name pid=$(cat "$PID_DIR/$name.pid")"
    else
      echo "DOWN $name"
    fi
  done
}

main() {
  local cmd="${1:-}"
  case "$cmd" in
    start)
      local slug="${2:-${POLYMARKET_SLUG:-}}"
      start_all "$slug"
      ;;
    stop) stop_all ;;
    clean) clean_all ;;
    status) status_all ;;
    -h|--help|help|"") usage ;;
    *)
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
