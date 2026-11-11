# Live evidence — LatchGuard

Contract: `latchguard-v4-ba2dcd.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract keys: **0** (deployer key deleted before the demo)
Deployed code hash equals local wasm SHA-256 `fcd75509…` (byte-for-byte).
Demo policy: per-tx 0.50 NEAR, rolling 60s window cap 1.00 NEAR, interval 0, timelock 30s.

## Scenario order (as executed)

1. **Timelocked allowlist.** Propose `testnet` → `get_allowlist` empty; after 30s
   `apply_pending_allowlist()` → `["testnet"]`.
   Tx: `E6hiWaizg6aQfNd4oGVLU4v7UWaw456oh4KvrfUfxhps`,
   `9vKqeyNqfK2j2PzqisZr4aV8kajo8zJhGJxJtM1q6hXk`.
2. **Under-cap attacker.** Signed 0.40 NEAR to `attacker.testnet` →
   `recipient_not_allowlisted`. Tx: `3GyU7JmBK1YjzYrUaEfyGUM3qZm3bEkDEUrueXkqkcqF`.
3. **Legit payments.** 0.40 nonce=2 settled
   (`G2sMg8zL2NeGQwuBoXHTS88eY1AWgaWgayWYVLBrZcVW`); 0.40 nonce=3 settled
   (`2uiqa4TEp4CUEz6A6GzSpfBq9etB8zrk9miWQY3pm7qp`).
4. **Rolling cap.** Third 0.40 inside 60s → `window_cap_exceeded`
   (`FbtoU5tXpyUkyH3b23ZLnjshrmp3beBxpmxyHV4Ru12E`).
5. **Window frees.** After 61s, 0.40 nonce=5 settles
   (`6iKfs7RVc2uT4xckdXBFLfVuRpsSo57oaH2mx1oTCUZ3`). Native access keys cannot do this.
6. **Tamper.** Signature for 0.40, call asks 0.50 → `bad_signature`
   (`7r4uPCUsq85SJSc9n6HgkUZoAuji3TuY4BDXAcAf8MFR`).
7. **Replay.** nonce=2 again → `nonce_replay`
   (`75HEzyWwGG9VEi4fnjpcA2UyayV6Tpz28hkHuLV1uJqQ`).
8. **Expiry.** `valid_until=now-10` → `expired_intent`
   (`2A2Sra3hQmruiVVYRAFYqbMA7nLLCkDHmDYFhSkuz1c5`).
9. **Overspend.** 0.60 → `per_tx_cap_exceeded`
   (`2zYPuZZqGZcDKmE3G2cSe93B9CsStVYJvjsqNgavMKoE`).
10. **Timelocked loosening.** Propose 1.00/2.00
    (`2gPNQ8pvYteebKiQcnpwQDVLf9ZixnjoGMxABdE5ZKjS`); 0.70 still blocked
    (`GBXh6RyTyUHszUv7d1xbuBs7d3dXxQnX42iLy9k2RVoV`); after 30s apply
    (`7MAcayXb8xrPTP5eCAyReLobgZG4Kxi49epQDB2VfZ3K`); 0.70 settles
    (`YNKc88se8XepFKQ8ZRDgy73xT3qEbjfsJT1RrN16oMx`).
11. **Lockdown.** Deployer key deleted
    (`7TFYWt3QPaU2V1CM56HLx9LpjetRHPzZx69LDAjHZWxX`); RPC key list empty.
12. **Pause / unpause.** Pause instant (`3oaGA2ZB6yvJkS9erhcrWHjeugHE1vVB23dt991gwZ6H`);
    unpause requested, still paused (`33nqSdr9kyyfef7FcRbRZkXxNsiB1zVvY3J4kQi4YdH2`);
    applied after 30s, unpaused (`Dcto4CLUhUADbC3vR3N4EWj9cbVwbPc1crkyC2HREDrF`).

## Views after the run

```
get_policy        = [1.00 NEAR per-tx, 2.00 NEAR window cap, 0 interval, 60s window]
get_spent_in_window = "700000000000000000000000"  (0.70 NEAR)
suggested_nonce   = 11
pending_policy    = null
is_paused         = false
```

## Reproduce

- Read-only: `bash scripts/verify-testnet.sh`
- Adversarial, with a freshly generated agent key: `bash scripts/attack-demo.sh <contract> <owner>`

Raw terminal output: `docs/attack-demo-output.txt`.
