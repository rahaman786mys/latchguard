#!/usr/bin/env bash
# Read-only: fetch recorded testnet transactions and print their outcome.
# Useful for a demo video without waiting for timelocks.
set -euo pipefail

python3 - <<'PY'
import json, urllib.request

RPC = "https://rpc.testnet.near.org"
SENDER = "agent-guard-owner-e714a8.testnet"
CASES = [
    ("under-cap attacker blocked", "3GyU7JmBK1YjzYrUaEfyGUM3qZm3bEkDEUrueXkqkcqF"),
    ("signed 0.40 settled", "G2sMg8zL2NeGQwuBoXHTS88eY1AWgaWgayWYVLBrZcVW"),
    ("window cap blocked", "FbtoU5tXpyUkyH3b23ZLnjshrmp3beBxpmxyHV4Ru12E"),
    ("after window settled", "6iKfs7RVc2uT4xckdXBFLfVuRpsSo57oaH2mx1oTCUZ3"),
    ("tamper blocked", "7r4uPCUsq85SJSc9n6HgkUZoAuji3TuY4BDXAcAf8MFR"),
    ("replay blocked", "75HEzyWwGG9VEi4fnjpcA2UyayV6Tpz28hkHuLV1uJqQ"),
    ("expired blocked", "2A2Sra3hQmruiVVYRAFYqbMA7nLLCkDHmDYFhSkuz1c5"),
    ("overspend blocked", "2zYPuZZqGZcDKmE3G2cSe93B9CsStVYJvjsqNgavMKoE"),
    ("loosening queued", "2gPNQ8pvYteebKiQcnpwQDVLf9ZixnjoGMxABdE5ZKjS"),
    ("still blocked (old cap)", "GBXh6RyTyUHszUv7d1xbuBs7d3dXxQnX42iLy9k2RVoV"),
    ("loosening applied", "7MAcayXb8xrPTP5eCAyReLobgZG4Kxi49epQDB2VfZ3K"),
    ("0.70 settled", "YNKc88se8XepFKQ8ZRDgy73xT3qEbjfsJT1RrN16oMx"),
    ("pause instant", "3oaGA2ZB6yvJkS9erhcrWHjeugHE1vVB23dt991gwZ6H"),
    ("unpause queued", "33nqSdr9kyyfef7FcRbRZkXxNsiB1zVvY3J4kQi4YdH2"),
    ("unpause applied", "Dcto4CLUhUADbC3vR3N4EWj9cbVwbPc1crkyC2HREDrF"),
]

def status(tx_hash):
    body = {"jsonrpc": "2.0", "id": "e", "method": "tx",
            "params": {"tx_hash": tx_hash, "sender_account_id": SENDER, "wait_until": "FINAL"}}
    req = urllib.request.Request(RPC, data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"})
    return json.load(urllib.request.urlopen(req, timeout=30))

for label, tx_hash in CASES:
    try:
        d = status(tx_hash)
        res = d.get("result", {})
        outcome = res.get("status", {})
        ok = "SuccessValue" in outcome
        reason = ""
        if not ok and "Failure" in outcome:
            reason = json.dumps(outcome["Failure"])[:120]
        logs = []
        for r in res.get("receipts_outcome", []):
            for line in r.get("outcome", {}).get("logs", []):
                if "AGENT_GUARD_" in line:
                    logs.append(line.strip())
        detail = logs[0] if logs else reason
        blocked = any("AGENT_GUARD_BLOCK" in line for line in logs)
        kind = "OK" if ok else ("BLOCK" if blocked else "FAIL")
        print(f"{kind:<5} {label:<26} {tx_hash[:12]}…  {detail}")
    except Exception as e:
        print(f"ERR   {label:<26} {tx_hash[:12]}…  {e}")
PY
