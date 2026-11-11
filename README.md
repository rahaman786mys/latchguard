# LatchGuard

**Signature-bound, policy-enforced payments for AI agents on NEAR.**
Track 06 — AI Security & Defense.

LatchGuard, deployed as `latchguard-v4-ba2dcd.testnet`.
Earlier development deployments used `agent-guard-*` / `latchguard-ef3931` IDs.

> A NEAR function-call key restricts *which method* an agent may call. It does not
> bind the payment's recipient and amount. LatchGuard requires the agent to sign an
> intent containing `(recipient, amount, nonce, valid_until)`. The contract verifies
> that signature, rejects reused nonces and expired intents, and enforces the
> owner's spending policy before releasing funds.

If a relay, RPC path, or host process rewrites the recipient or amount **after
signing**, execution fails with `bad_signature`. Replayed intents fail with
`nonce_replay`; stale ones with `expired_intent`. Correctly signed payments still
have to pass the allowlist, the per-tx cap, the rolling-window budget, and the
velocity limit.

## Live evidence (NEAR testnet)

Contract: `latchguard-v4-ba2dcd.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract account keys: **0** (deployer key deleted)
Demo policy: per-tx 0.50 NEAR, rolling **60s** window cap 1.00 NEAR, interval 0,
timelock **30s**. (Production: 24h window, 24–48h timelock; see below.)

| Attempt | Result | Transaction |
|---|---|---|
| Allowlist add (timelocked) | pending → applied after 30s | `E6hiWaizg6aQfNd4oGVLU4v7UWaw456oh4KvrfUfxhps`, `9vKqeyNqfK2j2PzqisZr4aV8kajo8zJhGJxJtM1q6hXk` |
| **Under-cap 0.40 NEAR to non-allowlisted attacker** | `recipient_not_allowlisted` | `3GyU7JmBK1YjzYrUaEfyGUM3qZm3bEkDEUrueXkqkcqF` |
| Signed 0.40 NEAR (nonce=2) | settled | `G2sMg8zL2NeGQwuBoXHTS88eY1AWgaWgayWYVLBrZcVW` |
| Signed 0.40 NEAR (nonce=3) | settled | `2uiqa4TEp4CUEz6A6GzSpfBq9etB8zrk9miWQY3pm7qp` |
| Third 0.40 inside 60s | `window_cap_exceeded` | `FbtoU5tXpyUkyH3b23ZLnjshrmp3beBxpmxyHV4Ru12E` |
| 0.40 after the window | settled | `6iKfs7RVc2uT4xckdXBFLfVuRpsSo57oaH2mx1oTCUZ3` |
| Signature for 0.40, call asks 0.50 | `bad_signature` | `7r4uPCUsq85SJSc9n6HgkUZoAuji3TuY4BDXAcAf8MFR` |
| Replay nonce=2 | `nonce_replay` | `75HEzyWwGG9VEi4fnjpcA2UyayV6Tpz28hkHuLV1uJqQ` |
| Expired intent | `expired_intent` | `2A2Sra3hQmruiVVYRAFYqbMA7nLLCkDHmDYFhSkuz1c5` |
| Overspend 0.60 NEAR | `per_tx_cap_exceeded` | `2zYPuZZqGZcDKmE3G2cSe93B9CsStVYJvjsqNgavMKoE` |
| Owner proposes higher caps | pending; 0.70 still blocked | `2gPNQ8pvYteebKiQcnpwQDVLf9ZixnjoGMxABdE5ZKjS`, `GBXh6RyTyUHszUv7d1xbuBs7d3dXxQnX42iLy9k2RVoV` |
| Policy applied after 30s | 0.70 settles | `7MAcayXb8xrPTP5eCAyReLobgZG4Kxi49epQDB2VfZ3K`, `YNKc88se8XepFKQ8ZRDgy73xT3qEbjfsJT1RrN16oMx` |
| Deployer key deleted | RPC key list empty | `7TFYWt3QPaU2V1CM56HLx9LpjetRHPzZx69LDAjHZWxX` |
| Pause (instant) | `is_paused=true` | `3oaGA2ZB6yvJkS9erhcrWHjeugHE1vVB23dt991gwZ6H` |
| Unpause requested (timelocked) | still paused | `33nqSdr9kyyfef7FcRbRZkXxNsiB1zVvY3J4kQi4YdH2` |
| Unpause applied after 30s | `is_paused=false` | `Dcto4CLUhUADbC3vR3N4EWj9cbVwbPc1crkyC2HREDrF` |

Explorer: `https://explorer.testnet.near.org/transactions/<hash>`
Read-only check: `bash scripts/verify-testnet.sh`.
Full adversarial run: `bash scripts/attack-demo.sh <contract> <owner>` (generates
an ephemeral agent key; no seed is committed).

## Why a contract, not just a function-call key?

| Capability | Function-call access key | Safe allowance / 4337 session key | LatchGuard |
|---|---|---|---|
| Restrict contract method | yes | n/a | yes (`pay`/`pay_signed` only) |
| Restrict recipients | no | yes | yes |
| Rolling window budget + velocity | no | partial | yes, configurable window |
| Bind recipient + amount to agent signature | no | delegate-signed message | **yes, with expiry + nonce** |
| Stop host-side rewrite after signing | no | partial | **yes** |
| Timelocked loosening | n/a | varies | **yes** |
| Agent needs NEAR for gas | yes | varies | **no** — any relayer submits; signature authorizes |

Gasless agents: `pay_signed` does not gate `predecessor_account_id`; the agent's
signature is the authorization. Relayer censorship is a liveness assumption.

## Policy

| Control | Meaning |
|---|---|
| Signed intent | exact `(recipient, amount, nonce, valid_until)` signed by the agent |
| Replay protection | nonces single-use on-chain; `suggested_nonce()` is advisory |
| Expiry | `valid_until` bounds signature lifetime |
| Allowlist | additions timelocked; removals instant |
| Per-transaction cap | max for one payment |
| Rolling window | sum over the trailing `window_secs` (60s demo / 24h prod) |
| Velocity | minimum seconds between payments |
| Loosening timelock | higher caps, narrower window, added recipients, and unpause wait `delay_secs`; tightening and pause are instant |
| Spend-log bound | 2048 entries; when full with in-window entries, payments fail closed (`spend_log_full`) |
| Balance check | never attempts to send more than it holds |

## Timelock bypass analysis

These are the ways a timelock usually fails; each is closed, tested, or disclosed:

1. **Window direction.** A narrower window with the same cap allows more per
   second. The classifier compares `daily_cap / window_secs` by checked
   cross-multiplication (`core::outflow_rate_le`), so narrowing queues.
   Note: shrinking the window also ages out spend measured against the old,
   longer window. The classifier allows a proportional cap cut as tightening, but
   a retroactive shrink can free budget early. Production should require
   `new.window_secs >= old.window_secs` for the instant path.
   Test: `narrowing_window_is_timelocked`.
2. **Burst vs sustained rate.** Equal rate can hide a 1440× larger burst (1 NEAR
   per 60s vs 1440 NEAR per day). `core::is_tightening` therefore requires **all**
   axes non-increasing: per-tx cap, daily cap (burst), rate, and velocity.
   Tests: `classifier_rejects_burst_increase_with_equal_rate`,
   property test `classifier_matches_all_loosening_axes`.
3. **Overflow.** Rate products use `checked_mul`; on overflow the change is
   classified as loosening (fail closed). Zero windows are rejected at policy
   set/init. Test: `overflow_fails_closed`.
4. **Delay cannot be lowered.** `delay_secs` is set in `#[init]` and has no
   setter. The demo value is 30s; production recommendation is 24–48h.
5. **Log eviction.** The rolling sum is recomputed from a bounded log; evicting
   in-window entries would undercount. When the log is full after pruning, the
   contract rejects (`spend_log_full`) instead of evicting. Only authorized
   payments are recorded: policy rejections panic and revert before the log write
   (and do not burn the nonce); an async transfer failure still consumes budget
   and nonce. With a production velocity of 60s, at most 1,440 entries fit in a
   24h window (< 2,048), so the bound is unreachable; the 60s demo window with
   interval 0 can fill it (accepted liveness DoS, disclosed).
6. **Pending vs tightening.** Any tightening action — a lower policy, an allowlist
   removal, or pause — clears **all** pending loosening slots (policy, allowlist,
   unpause). Pending entries can also be cancelled explicitly. Owner key rotation
   (`set_agent_key`) is intentionally instant for incident response; the owner is
   trusted and can already call `pay`. Tests:
   `tightening_clears_all_pending_slots`, `pause_clears_all_pending_slots`,
   `reproposal_resets_timer`.
7. **Intent lifetime and nonce storage.** Policy rejections panic and revert, so a
   rejected signature stays valid until `valid_until`; agents should sign short
   expiries (the demo uses minutes). No contract-enforced maximum TTL exists.
   Nonces are an arbitrary-value set, not sequential, and grow one entry per
   authorized payment; production should use a monotonic counter.

Delayed unpause is part of the same asymmetry: pause is instant, unpause waits the
timelock (`apply_unpause`), so a compromised owner cannot flash-unpause. Pending
requests have no automatic expiry; the owner can cancel them, and anyone can apply
them only after the delay.

## Threat model

**In scope**

- Host/relayer rewrites recipient or amount after signing; replay; delayed execution.
- Prompt-injected agent choosing an out-of-policy payment: blocked by allowlist,
  per-tx cap, rolling budget or velocity.
- Compromised agent key: loss bounded by policy; owner pause instant; loosening
  and unpause timelocked.

**Out of scope (stated, not implied)**

- An agent signing a malicious **in-policy** payment to an **allowlisted**
  recipient. LatchGuard enforces spending policy, not intent legitimacy;
  recipient reputation/allowlist curation is future work.
- Exfiltrated signing key combined with in-policy payments: bounded, not prevented.
- Owner key compromise before a timelock matures; owner is a multisig/hardware
  account in production.
- Liveness: the host may refuse to submit.
- Failed transfers: `Promise::transfer` to an invalid recipient fails after the
  call commits; the budget/nonce are consumed. Retry with a new nonce.

## Prompt-injection harness

`cargo run --example agent_injection` deterministically replays the post-injection
tool-call state of an agent reading a poisoned invoice, with **no external API key**:
attacker address (blocked), overspend (blocked), legitimate invoice (settles),
replay (blocked). It is LLM-agnostic by design: LatchGuard sits beneath any agent
runtime as the signing/settlement boundary. It does not claim to solve prompt
injection upstream.

## Layout

```
src/core.rs          pure policy engine (rolling window, outflow-rate monotonicity)
src/intent.rs        intent message + ed25519 verification (env on wasm)
src/lib.rs           near-sdk contract: state, owner controls, pay/pay_signed, views
examples/agent_demo.rs        offline policy + signature demo
examples/agent_injection.rs   deterministic injection harness
examples/agent_sign.rs        prints a key + intent signature (random or AGENT_SEED)
tests/fuzz.rs        property tests: policy invariants + signature mutations
scripts/attack-demo.sh        adversarial live run (ephemeral key)
scripts/verify-testnet.sh     read-only verification of deployed state
```

## Quickstart

```bash
cargo test                        # 41 tests (38 unit + 3 property)
cargo run --example agent_demo
cargo run --example agent_injection
cargo build --release --target wasm32-unknown-unknown
```

No fixed private key is committed. `agent_sign` generates a random key unless
`AGENT_SEED` is set; the live demo script generates an ephemeral seed per run.

## Contract interface

```text
new(owner, per_tx_cap, daily_cap, min_interval_secs, window_secs, delay_secs)
set_policy(per_tx_cap, daily_cap, min_interval_secs, window_secs)   owner; timelocked if loosening
apply_pending_policy() / cancel_pending_policy()
set_allowlist(account, allowed)                                      removal instant, addition timelocked
apply_pending_allowlist()
set_agent_key(public_key)                                            owner
pause()                                                              owner, instant
unpause() / apply_unpause()                                          unpause timelocked
deposit()
pay(recipient, amount, memo)
pay_signed(recipient, amount, nonce, valid_until, signature)
can_pay(recipient, amount) -> (bool, reason)
get_policy / get_allowlist / get_spent_in_window / is_paused / owner
agent_key / nonce_used / suggested_nonce / pending_policy / pending_allowlist
pending_unpause / window_secs / delay_secs
```

## Deployment notes

The contract account's deployer key is deleted after deployment (RPC shows 0
keys) and the code exposes no self-upgrade or key-addition method: **the code is
immutable, while policy remains mutable through the timelocked owner**. A bug
cannot be patched in place and there is no admin withdrawal. Recovery means
deploying a new contract and migrating balances; deliberate for the hackathon.
The separate owner account keeps policy/pause authority and is a trusted party
(owner compromise is out of scope; the timelock only delays it).

Demo parameters are 60s window / 30s timelock to make the behaviour observable in
a short video. Production should use a 24h window and a 24–48h timelock.

Build artifact from the deployed source tree: 245,410 bytes, SHA-256
`add2079b5a4449cd10d6b5cd08e0627daee7bae1f9a7102c61d6cd80f3737b1e`
(`cargo build --release --target wasm32-unknown-unknown`). No contract source was
changed after deployment; only docs and scripts.

NEAR `view_account` for `latchguard-v4-ba2dcd.testnet` reports
`code_hash = add2079b5a4449cd10d6b5cd08e0627daee7bae1f9a7102c61d6cd80f3737b1e`
after Base58→hex decoding, byte-for-byte equal to the local build. Because the
account has 0 keys and the code has no deploy/key-add path, that code cannot
change. `scripts/verify-testnet.sh` decodes the Base58 `code_hash`, prints both
encodings, and compares it to this expected value.

## Limitations / roadmap

- Native NEAR only; NEP-141 and NEAR Intents next.
- Spend log is a bounded `Vec` (2048); production would use a ring buffer with an
  incremental running sum.
- No owner approval queue for over-cap payments (binary allow/block today).
- No SDK yet; `agent_sign` documents the message format.
- Not audited; testnet only.

## License

MIT.
