#!/usr/bin/env bash
# Read-only verification of the deployed LatchGuard contract. No credentials.
set -euo pipefail
contract="${1:-latchguard-v3-29aa85.testnet}"

python3 - "$contract" <<'PY'
import base64, json, sys, urllib.request

contract = sys.argv[1]
RPC = "https://rpc.testnet.near.org"

def rpc(method, params):
    body = {"jsonrpc": "2.0", "id": "verify", "method": method, "params": params}
    req = urllib.request.Request(
        RPC, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"}
    )
    return json.load(urllib.request.urlopen(req, timeout=30))

keys = rpc("query", {
    "request_type": "view_access_key_list",
    "finality": "final",
    "account_id": contract,
})["result"]["keys"]
print(f"contract={contract}")
print(f"key_count={len(keys)}")
if len(keys) != 0:
    print("WARNING: contract account still has keys")

def view(method, args=None):
    payload = base64.b64encode(json.dumps(args or {}).encode()).decode()
    res = rpc("query", {
        "request_type": "call_function",
        "finality": "final",
        "account_id": contract,
        "method_name": method,
        "args_base64": payload,
    })["result"]
    return bytes(res["result"]).decode()

for method in ["get_policy", "get_spent_in_window", "suggested_nonce", "window_secs", "delay_secs", "is_paused"]:
    print(f"{method}={view(method)}")

key = view("agent_key")
print(f"agent_key_len={len(json.loads(key))}")
PY
