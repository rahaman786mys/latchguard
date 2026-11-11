# LatchGuard

**Signature-bound, policy-enforced payments for AI agents on NEAR.**
Track 06 — AI Security & Defense.

LatchGuard, deployed as `latchguard-v2-47d093.testnet`.
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

Contract: `latchguard-v2-47d093.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract account keys: **0** (deployer key deleted)
Demo policy: per-tx 0.50 NEAR, rolling **60s** window cap 1.00 NEAR, interval 0,
timelock **30s**. (Production: 24h window, 24–48h timelock; see below.)

| Attempt | Result | Transaction |
|---|---|---|
| Allowlist add (timelocked) | pending → applied after 30s | `H1kX7RfhkeSNC2CTcHQrx3u3Ct2MuDjFo4qoYCsEJjgs`, `6VAoPgrTkAYwusct4emHk4zxmkRd54URh8ys77W4B7rr` |
| **Under-cap 0.40 NEAR to non-allowlisted attacker** | `recipient_not_allowlisted` | `8bvJba7W9ukuJhsSCUpCeJ9pnpTMTxUaFYn33C117PP2` |
| Signed 0.40 NEAR (nonce=2) | settled | `FwMuBdYe8ZpPf2YWV24hjEvxuN9CitdimWfBjJGdPa2s` |
| Signed 0.40 NEAR (nonce=3) | settled | `CaQRHV4suzNkBT6qc8dS9KTLqgDSaPAiWuG3gbwU9qpP` |
| Third 0.40 inside 60s | `window_cap_exceeded` | `EGxVG2KVYasvnHdxPsXi9TeTX6Gg4dcfYj9uXoRtT7bD` |
| 0.40 after the window | settled | `veqvyoLc3n2UAoNutzEws1svauK3fqUhwshuNs71nbj` |
| Signature for 0.40, call asks 0.50 | `bad_signature` | `F13jMCAgVc2ra3rZdLE7WaN1wVN8WhgDv1XJtYD7YbHK` |
| Replay nonce=2 | `nonce_replay` | `CL7RXhHyfcausrdaB4ccK7cYc1V96o2dNnAubcfJgfio` |
| Expired intent | `expired_intent` | `94GBVCjLU5CpTkfL3pyYuFXqPn2b5mCAtKQ6Lx1BKzGC` |
| Overspend 0.60 NEAR | `per_tx_cap_exceeded` | `CAVPS4zGGj8aU9JPp4WJHrGHT57w3r4uhp8YVZmbbxnW` |
| Owner proposes higher caps | pending; 0.70 still blocked | `6G4b3DF3ZZB3YQRVQTGJfhpMasVJtYsnbybNJ5F3mQow`, `Cd3DpcxRdFhFXjJWxiSY19vkWp5iVnR2qBq788S97b1p` |
| Policy applied after 30s | 0.70 settles | `vcpGd72PsZ7f8fz3jcvCZz6bAS5tvnMk5TcZsgP82uP`, `2CfRQHRME4QCjhuXLjW4qpb4boQ5svNwYeVhRLkYW94S` |
| Deployer key deleted | RPC key list empty | `3xgM1EmuDWeYtuwGMBLf1KuXWe19BVFBXC3693moyxXp` |
| Pause (instant) | `is_paused=true` | `7NdoiZEA9jC5tANk1M8djbBCodkE9cgmDRQFTsDyNhn4` |
| Unpause requested (timelocked) | still paused | `2TfjsyVvP3ffZ9aMorTcAwBzUcmw8BtRLpSf85tEc64K` |
| Unpause applied after 30s | `is_paused=false` | `5Vps6w7vF7DJzFAjDX5BqsVV32TPQKQy1iRN7VYg73mA` |

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

These are the four ways a timelock usually fails; each is closed and tested:

1. **Window direction.** A narrower window with the same cap allows more per
   second. `is_tightening` compares `daily_cap / window_secs` by cross-multiplication
   (`core::outflow_rate_le`), so narrowing is queued as loosening; widening is
   tightening. Test: `narrowing_window_is_timelocked`.
2. **Delay cannot be lowered.** `delay_secs` is set in `#[init]` and has no setter.
   The demo value is 30s; production recommendation is 24–48h.
3. **Log eviction.** The rolling sum is recomputed from a bounded log; evicting
   in-window entries would reduce the sum. When the log is full after pruning, the
   contract rejects (`spend_log_full`) instead of evicting.
4. **Pending vs tightening.** A tightening change clears any pending loosening
   before applying; pending policy can be cancelled by the owner. Test:
   `tightening_clears_pending_loosening`.

Delayed unpause is part of the same asymmetry: pause is instant, unpause waits the
timelock (`apply_unpause`), so a compromised owner cannot flash-unpause.

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
cargo test                        # 34 tests (32 unit + 2 property)
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
keys). Consequence: **the contract is immutable** — a bug cannot be patched in
place, and there is no admin withdrawal. Recovery means deploying a new contract
and migrating balances; deliberate for the hackathon. The separate owner account
keeps policy/pause authority.

Demo parameters are 60s window / 30s timelock to make the behaviour observable in
a short video. Production should use a 24h window and a 24–48h timelock.

Build artifact from the deployed source tree: 245,163 bytes, SHA-256
`2e047bf8f5d095298f825864ab7f23b6dd60639a4e0486718272d9fcf5ec51b8`
(`cargo build --release --target wasm32-unknown-unknown`). No contract source was
changed after deployment; only docs and the read-only verify script.

## Limitations / roadmap

- Native NEAR only; NEP-141 and NEAR Intents next.
- Spend log is a bounded `Vec` (2048); production would use a ring buffer with an
  incremental running sum.
- No owner approval queue for over-cap payments (binary allow/block today).
- No SDK yet; `agent_sign` documents the message format.
- Not audited; testnet only.

## License

MIT.
