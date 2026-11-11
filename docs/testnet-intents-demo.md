# Live evidence — LatchGuard

Contract: `latchguard-v2-47d093.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract keys: **0** (deployer key deleted before the demo)
Demo policy: per-tx 0.50 NEAR, rolling 60s window cap 1.00 NEAR, interval 0, timelock 30s.

## Scenario order (as executed)

1. **Timelocked allowlist.** Propose `testnet` → `get_allowlist` empty; after 30s
   `apply_pending_allowlist()` → `["testnet"]`.
   Tx: `H1kX7RfhkeSNC2CTcHQrx3u3Ct2MuDjFo4qoYCsEJjgs`,
   `6VAoPgrTkAYwusct4emHk4zxmkRd54URh8ys77W4B7rr`.
2. **Under-cap attacker.** Signed 0.40 NEAR to `attacker.testnet` →
   `recipient_not_allowlisted`. Tx: `8bvJba7W9ukuJhsSCUpCeJ9pnpTMTxUaFYn33C117PP2`.
3. **Legit payments.** 0.40 nonce=2 settled
   (`FwMuBdYe8ZpPf2YWV24hjEvxuN9CitdimWfBjJGdPa2s`); 0.40 nonce=3 settled
   (`CaQRHV4suzNkBT6qc8dS9KTLqgDSaPAiWuG3gbwU9qpP`).
4. **Rolling cap.** Third 0.40 inside 60s → `window_cap_exceeded`
   (`EGxVG2KVYasvnHdxPsXi9TeTX6Gg4dcfYj9uXoRtT7bD`).
5. **Window frees.** After 61s, 0.40 nonce=5 settles
   (`veqvyoLc3n2UAoNutzEws1svauK3fqUhwshuNs71nbj`). Native access keys cannot do this.
6. **Tamper.** Signature for 0.40, call asks 0.50 → `bad_signature`
   (`F13jMCAgVc2ra3rZdLE7WaN1wVN8WhgDv1XJtYD7YbHK`).
7. **Replay.** nonce=2 again → `nonce_replay`
   (`CL7RXhHyfcausrdaB4ccK7cYc1V96o2dNnAubcfJgfio`).
8. **Expiry.** `valid_until=now-10` → `expired_intent`
   (`94GBVCjLU5CpTkfL3pyYuFXqPn2b5mCAtKQ6Lx1BKzGC`).
9. **Overspend.** 0.60 → `per_tx_cap_exceeded`
   (`CAVPS4zGGj8aU9JPp4WJHrGHT57w3r4uhp8YVZmbbxnW`).
10. **Timelocked loosening.** Propose 1.00/2.00
    (`6G4b3DF3ZZB3YQRVQTGJfhpMasVJtYsnbybNJ5F3mQow`); 0.70 still blocked
    (`Cd3DpcxRdFhFXjJWxiSY19vkWp5iVnR2qBq788S97b1p`); after 30s apply
    (`vcpGd72PsZ7f8fz3jcvCZz6bAS5tvnMk5TcZsgP82uP`); 0.70 settles
    (`2CfRQHRME4QCjhuXLjW4qpb4boQ5svNwYeVhRLkYW94S`).
11. **Lockdown.** Deployer key deleted
    (`3xgM1EmuDWeYtuwGMBLf1KuXWe19BVFBXC3693moyxXp`); RPC key list empty.
12. **Pause / unpause.** Pause instant (`7NdoiZEA9jC5tANk1M8djbBCodkE9cgmDRQFTsDyNhn4`);
    unpause requested, still paused (`2TfjsyVvP3ffZ9aMorTcAwBzUcmw8BtRLpSf85tEc64K`);
    applied after 30s, unpaused (`5Vps6w7vF7DJzFAjDX5BqsVV32TPQKQy1iRN7VYg73mA`).

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
