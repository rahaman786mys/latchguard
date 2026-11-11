# DEMO — reproduce in 5 minutes

## Read-only against the deployed contract (no credentials)

```bash
bash scripts/verify-testnet.sh
```

Expected: `key_count=0`, policy tuple with a 60s window, spent window value,
`suggested_nonce`, `window_secs=60`, `delay_secs=30`, `is_paused=false`.

## Offline (no chain, no API keys)

```bash
cargo test                        # 41 tests
cargo run --example agent_demo    # policy + signed intent demo
cargo run --example agent_injection
```

`agent_injection` is a deterministic replay of the post-injection tool-call state.
It requires no LLM key and is not claimed to be a live model.

## On-chain evidence

See `docs/testnet-intents-demo.md` for the full transaction table, or the README
table. Key outcomes:

- 0.40 NEAR to a non-allowlisted attacker → `recipient_not_allowlisted`
- 0.40 NEAR signed and allowed; third payment inside 60s → `daily_cap_exceeded`
- after 61s, 0.40 NEAR settles again (rolling window)
- tampered amount → `bad_signature`
- replay → `nonce_replay`
- stale signature → `expired_intent`
- overspend → `per_tx_cap_exceeded`
- owner loosening → pending, blocked until `apply_pending_policy()`
- deployer key deleted; RPC shows 0 keys; owner pause still works

## Recording script (60–90s)

1. `bash scripts/verify-testnet.sh` — show `key_count=0`.
2. `cargo run --example agent_injection` — attacker and overspend blocked, legit settles, replay blocked.
3. On the deployed contract: under-cap 0.40 to attacker → `recipient_not_allowlisted`; tampered call → `bad_signature`; replay → `nonce_replay`.
4. Legit 0.40 settles; show explorer tx.
5. Rolling window: two 0.40 settle, third → `window_cap_exceeded`; after 61s it settles again.
6. Timelock: propose higher cap → still blocked → applied after 30s → settles.
7. Pause instant; unpause queued; `key_count=0`.

Say plainly: the injection harness is deterministic; LatchGuard is the
signing/settlement boundary beneath any agent runtime.
