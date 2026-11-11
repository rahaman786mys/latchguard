#!/usr/bin/env bash
# Read-only verification of the deployed LatchGuard contract. No credentials.
set -euo pipefail
contract="${1:-latchguard-v4-ba2dcd.testnet}"
expected_code_hash="add2079b5a4449cd10d6b5cd08e0627daee7bae1f9a7102c61d6cd80f3737b1e"

python3 - "$contract" "$expected_code_hash" <<'PY'
import base64, json, sys, urllib.request

contract, expected = sys.argv[1], sys.argv[2]
RPC = "https://rpc.testnet.near.org"
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

def base58_decode(s: str) -> bytes:
    n = 0
    for ch in s:
        n = n * 58 + ALPHABET.index(ch)
    raw = n.to_bytes((n.bit_length() + 7) // 8, "big")
    return b"\x00" * (len(s) - len(s.lstrip("1"))) + raw

def rpc(method, params):
    body = {"jsonrpc": "2.0", "id": "verify", "method": method, "params": params}
    req = urllib.request.Request(
        RPC, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"}
    )
    return json.load(urllib.request.urlopen(req, timeout=30))

account = rpc("query", {
    "request_type": "view_account", "finality": "final", "account_id": contract
})["result"]
keys = rpc("query", {
    "request_type": "view_access_key_list", "finality": "final", "account_id": contract
})["result"]["keys"]

code_hex = base58_decode(account["code_hash"]).hex()
C = "\x1b[36;1m"
G = "\x1b[32;1m"
R = "\x1b[31;1m"
B = "\x1b[1m"
X = "\x1b[0m"
print(f"{C}contract{X}={B}{contract}{X}")
keys_ok = len(keys) == 0
print(f"{C}key_count{X}={G if keys_ok else R}{len(keys)}{X}")
print(f"{C}code_hash_base58{X}={account['code_hash']}")
print(f"{C}code_hash_hex{X}={code_hex}")
print(f"{C}code_hash_expected{X}={expected}")
match = code_hex == expected
print(f"{C}code_hash_match{X}={G if match else R}{'MATCH' if match else 'MISMATCH'}{X}")

def view(method, args=None):
    payload = base64.b64encode(json.dumps(args or {}).encode()).decode()
    res = rpc("query", {
        "request_type": "call_function", "finality": "final",
        "account_id": contract, "method_name": method, "args_base64": payload,
    })["result"]
    return bytes(res["result"]).decode()

for method in ["get_policy", "get_spent_in_window", "suggested_nonce", "window_secs", "delay_secs", "is_paused", "pending_policy", "pending_allowlist", "pending_unpause"]:
    print(f"{C}{method}{X}={view(method)}")

print(f"{C}agent_key_len{X}={len(json.loads(view('agent_key')))}")
PY
