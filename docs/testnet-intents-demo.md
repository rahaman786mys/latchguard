# Live evidence — LatchGuard

Contract: `latchguard-ef3931.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract keys: **0** (deployer key deleted)
Policy: per-tx 0.50 NEAR, rolling 60s window cap 1.00 NEAR, interval 0, timelock 30s.

## Scenario order (as executed)

1. **Timelocked allowlist.** `set_allowlist(testnet, true)` → pending; `get_allowlist` empty.
   After 30s, `apply_pending_allowlist()` → `["testnet"]`.
   Tx: `2kDZwP3bdbRiBkE86JBfztmGLFdu2zRSDExaj2QaXZCg`, `Cf4tuBkUSgRUDVg4XgmeJVRwQydNW2u3Wcykv86NRZ8j`.
2. **Under-cap attacker.** Signed 0.40 NEAR to `attacker.testnet` (not allowlisted)
   → `AGENT_GUARD_BLOCK reason=recipient_not_allowlisted`.
   Tx: `4dwtBu3dGfEPy44ktHPYEUj4M4xvpKFpByEhu6hRywjC`.
3. **Legitimate payments.** 0.40 NEAR nonce=2 → settled
   (`HLvBFXGsV8Nai2b8h2xFGofBcUBZTvXPbThDtxVcmPQD`); 0.40 nonce=3 → settled
   (`Hfk9k2bm2QaVinL6u1EBvJWoF3hFgjiTcbsakZVRafvL`).
4. **Rolling cap.** Third 0.40 inside 60s → `daily_cap_exceeded`
   (`BNnz7zdsdXKanoNV66EVv1hN6EYSEvYrRZZSyQKossJo`).
5. **Window frees.** After 61s, 0.40 nonce=5 → settled
   (`KjcJfag7pgp7648A6fdTGZ4rS8qsaYJc2QgiNGYqkVt`). This is the capability a
   native function-call key cannot provide.
6. **Tamper.** Signature for 0.40, call asks 0.50, nonce=6 → `bad_signature`
   (`APVTkguTXnSvhxZR17h6ZcwGyJcWptRk8sNL26UsFcGi`).
7. **Replay.** nonce=2 again → `nonce_replay`
   (`BJSNeSYxgbjp5VqD8wvZ9tKVDjeuawg6gC4JNrvTNVPp`).
8. **Expiry.** `valid_until=now-10` → `expired_intent`
   (`B8MDtj9uNmUPGipCoY4y3j6HEfitYg9LhvWV2cHdsP1c`).
9. **Overspend.** 0.60 NEAR → `per_tx_cap_exceeded`
   (`5HVneude7PHHQu1PML1qfgQwfNrTYN7Q2t1v5N641xK`).
10. **Timelocked loosening.** `set_policy(1.00, 2.00, 0, 60)` → pending
    (`DMZsDsVB5WSqbhNuVofte5ZjDkSt7QZcfc2Hty3Cdq8t`); 0.70 still blocked
    (`8Y9jENMfsjw5vGaXyS3bQmWwjyzu32xcV856mod3zgqH`); after 30s
    `apply_pending_policy()` (`A7dj5BXvxsb2A6y5JZKTKi1v18TXmNEkncA9TUKFbJgE`);
    0.70 settles (`BBSSm75x7sWuXbs6keQpLmyWPYaFanSspFjjgtAGpXcE`).
11. **Lockdown.** Deployer key deleted
    (`2sjg9FwuQMFuZgifevAKQRArD8VyXihM9rMsrgqL6wm2`); RPC `view_access_key_list`
    → `key_count = 0`. Owner pause/unpause still works
    (`5ZbuuXcpneQ7b6hdF3qBi4mGPzNoMXYoP9imVu6ee9PY`,
    `83A2X9jC8WNiKPbUsGku6qToD4rCUtK41pg9Y2iY1StC`).

## Views after the run

```
get_spent_in_window = "700000000000000000000000"   (0.70 NEAR in the trailing 60s)
suggested_nonce      = 11
```

## Reproduce read-only

```bash
bash scripts/verify-testnet.sh
```

It checks the keyless account, the policy tuple, spent window, agent key length and
pause state via JSON-RPC. No credentials required.

Raw terminal output: `docs/testnet-intents-demo.txt`.
