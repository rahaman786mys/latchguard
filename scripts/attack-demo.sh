#!/usr/bin/env bash
# Adversarial demo against a deployed contract. Generates an ephemeral agent key
# (never committed), registers it through the owner, then runs the attacks.
#
# Usage: scripts/attack-demo.sh <contract> <owner>
set -u
export PATH="$HOME/bin:$PATH"
cd "$(dirname "$0")/.."

contract="${1:?contract}"
owner="${2:?owner}"
out="docs/attack-demo-output.txt"
seed=$(openssl rand -hex 32)
export AGENT_SEED="$seed"
now=$(date +%s)
until=$((now + 900))
mkdir -p docs

json_u8() { python3 -c "import sys;print('['+','.join(str(b) for b in bytes.fromhex(sys.argv[1]))+']')" "$1"; }
sign_for() { cargo run --quiet --example agent_sign -- "$contract" "$1" "$2" "$3" "$4"; }
call_signed() {
  near call "$contract" pay_signed "{\"recipient\":\"$1\",\"amount\":\"$2\",\"nonce\":$3,\"valid_until\":$4,\"signature\":$5}" \
    --use-account "$owner" --network-id testnet 2>&1 \
    | grep -E "Transaction ID|AGENT_GUARD_ALLOW|AGENT_GUARD_BLOCK" | head -2
}

{
  echo "LatchGuard — adversarial demo ($(date -u +%Y-%m-%dT%H:%M:%SZ))"
  echo "contract: $contract"
  echo "owner:    $owner"
  echo
  keyout=$(sign_for testnet 400000000000000000000000 1 "$until")
  pub=$(echo "$keyout" | sed -n 's/^pubkey=//p')
  echo "ephemeral agent pubkey: $pub"

  echo
  echo "== timelocked allowlist =="
  near call "$contract" set_allowlist '{"account":"testnet","allowed":true}' --use-account "$owner" --network-id testnet 2>&1 | grep -E "Transaction ID" | head -1
  echo "waiting 31s..."; sleep 31
  near call "$contract" apply_pending_allowlist '{}' --use-account "$owner" --network-id testnet 2>&1 | grep -E "Transaction ID" | head -1

  echo
  echo "== register agent key + fund =="
  near call "$contract" set_agent_key "{\"public_key\":$(json_u8 "$pub")}" --use-account "$owner" --network-id testnet 2>&1 | grep -E "Transaction ID" | head -1
  near call "$contract" deposit '{}' --use-account "$owner" --deposit 2 --network-id testnet 2>&1 | grep -E "Transaction ID" | head -1

  echo
  echo "== ATTACK: under-cap 0.40 to non-allowlisted attacker =="
  a=$(sign_for attacker.testnet 400000000000000000000000 1 "$until" | sed -n 's/^signature=//p')
  call_signed attacker.testnet 400000000000000000000000 1 "$until" "$(json_u8 "$a")"

  echo
  echo "== legit 0.40 (nonce=2) =="
  s2=$(sign_for testnet 400000000000000000000000 2 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 400000000000000000000000 2 "$until" "$(json_u8 "$s2")"

  echo
  echo "== second 0.40 (nonce=3) =="
  s3=$(sign_for testnet 400000000000000000000000 3 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 400000000000000000000000 3 "$until" "$(json_u8 "$s3")"

  echo
  echo "== third 0.40 (nonce=4) — should hit window cap =="
  s4=$(sign_for testnet 400000000000000000000000 4 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 400000000000000000000000 4 "$until" "$(json_u8 "$s4")"

  echo
  echo "waiting 61s for the rolling window..."; sleep 61
  echo "== 0.40 after the window (nonce=5) =="
  s5=$(sign_for testnet 400000000000000000000000 5 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 400000000000000000000000 5 "$until" "$(json_u8 "$s5")"

  echo
  echo "== tamper: signature for 0.40, call asks 0.50 (nonce=6) =="
  s6=$(sign_for testnet 400000000000000000000000 6 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 500000000000000000000000 6 "$until" "$(json_u8 "$s6")"

  echo
  echo "== replay nonce=2 =="
  call_signed testnet 400000000000000000000000 2 "$until" "$(json_u8 "$s2")"

  echo
  echo "== expired intent (nonce=7) =="
  past=$((now - 10))
  s7=$(sign_for testnet 400000000000000000000000 7 "$past" | sed -n 's/^signature=//p')
  call_signed testnet 400000000000000000000000 7 "$past" "$(json_u8 "$s7")"

  echo
  echo "== overspend 0.60 (nonce=8) =="
  s8=$(sign_for testnet 600000000000000000000000 8 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 600000000000000000000000 8 "$until" "$(json_u8 "$s8")"

  echo
  echo "== timelocked loosening: propose 1.00/2.00 =="
  near call "$contract" set_policy '{"per_tx_cap":"1000000000000000000000000","daily_cap":"2000000000000000000000000","min_interval_secs":0,"window_secs":60}' --use-account "$owner" --network-id testnet 2>&1 | grep -E "Transaction ID" | head -1
  echo "== 0.70 still blocked (nonce=9) =="
  s9=$(sign_for testnet 700000000000000000000000 9 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 700000000000000000000000 9 "$until" "$(json_u8 "$s9")"
  echo "waiting 31s..."; sleep 31
  near call "$contract" apply_pending_policy '{}' --use-account "$owner" --network-id testnet 2>&1 | grep -E "Transaction ID" | head -1
  echo "== 0.70 settles (nonce=10) =="
  s10=$(sign_for testnet 700000000000000000000000 10 "$until" | sed -n 's/^signature=//p')
  call_signed testnet 700000000000000000000000 10 "$until" "$(json_u8 "$s10")"

  echo
  echo "== views =="
  for m in get_policy get_spent_in_window suggested_nonce pending_policy is_paused; do
    echo -n "$m="
    near view "$contract" "$m" --network-id testnet 2>&1 | grep -A3 "return value" | tail -4 | tr '\n' ' '; echo
  done
} | tee "$out"
