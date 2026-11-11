# Live evidence — LatchGuard

Contract: `latchguard-v3-29aa85.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract keys: **0** (deployer key deleted before the demo)
Deployed code hash equals local wasm SHA-256 `fcd75509…` (byte-for-byte).
Demo policy: per-tx 0.50 NEAR, rolling 60s window cap 1.00 NEAR, interval 0, timelock 30s.

## Scenario order (as executed)

1. **Timelocked allowlist.** Propose `testnet` → `get_allowlist` empty; after 30s
   `apply_pending_allowlist()` → `["testnet"]`.
   Tx: `97kQnXhxaSVF4ieHzCr7F7svvMrtZrn7YGKPY7kmxnU2`,
   `G9FtJmS5VUtyqDkKm2PVqbrDrUBhyn37EpGbkSD9sB5E`.
2. **Under-cap attacker.** Signed 0.40 NEAR to `attacker.testnet` →
   `recipient_not_allowlisted`. Tx: `6Nn5HVfenriB3chNY5cVdAyBo8N741bqAVDwLKVVBJ9Q`.
3. **Legit payments.** 0.40 nonce=2 settled
   (`6b7tbE3AgSx417CXELv5dLqdjGqTtcaZeNKfxYXWUtVH`); 0.40 nonce=3 settled
   (`oQB92m4SCE7U67oLMXE1FoPUqJxBckbMP3eXsmqKSUJ`).
4. **Rolling cap.** Third 0.40 inside 60s → `window_cap_exceeded`
   (`DKk2vibuzM2sKJoyUiTzxw9GQYaLgSeB2Ax34yWrYCin`).
5. **Window frees.** After 61s, 0.40 nonce=5 settles
   (`HfpeQ8HXRFDtrH4wWd9cTXCW2nCzoxU6H6hQxxp613g5`). Native access keys cannot do this.
6. **Tamper.** Signature for 0.40, call asks 0.50 → `bad_signature`
   (`8WbzJzd4D9devwPQDnXiDYcHb9pXoek52jEJEFghkaua`).
7. **Replay.** nonce=2 again → `nonce_replay`
   (`9wB7HTLk5ejKxMU1wxatgPt3CuqEVgsRDBbSYwSKjVZ5`).
8. **Expiry.** `valid_until=now-10` → `expired_intent`
   (`9Q4861fGRU7j9YEJeH9EwJZUKMUiRRiGsc2b24tDnxmU`).
9. **Overspend.** 0.60 → `per_tx_cap_exceeded`
   (`EbMU7DFAEpj6RDwwzZKeZoLDoKtvECKUpgJarR1psrKE`).
10. **Timelocked loosening.** Propose 1.00/2.00
    (`AdA5qaU7pAAqVW5S1eEwLMmS2MyqwRMvqHMzkzyYJNj5`); 0.70 still blocked
    (`BT2PUdC7wqx7YV3iEHntUwpLGb7M8axY61TeEeec2Yeg`); after 30s apply
    (`6rGRS3GPkEaEsggS6PqEBpCT5YjUyRRrd3yESeNcRr73`); 0.70 settles
    (`7uzcEqZYDcHx6pcUrtwxa8hFb88aNB3oRN43pPquVE3J`).
11. **Lockdown.** Deployer key deleted
    (`9MDg2Lbx7tHA6hQfGRv4fpHLxDhRWfRPVyuNGzbynr3S`); RPC key list empty.
12. **Pause / unpause.** Pause instant (`6a3DXmwV9jUS7EthfZdhATqsAUgnWYRUuTEdztrtEdLY`);
    unpause requested, still paused (`2vu4zaR4i1xxrK8ZwNJG1ZM1pWttDuv6W31HbCPhi6rf`);
    applied after 30s, unpaused (`5S3Ck27RH8KSTdcehh57BFfqY6ry8bnQkU9Az8M7gytJ`).

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
