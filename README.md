# LatchGuard

**Signature-bound, policy-enforced payments for AI agents on NEAR.**
Track 06 — AI Security & Defense.

LatchGuard, deployed as `latchguard-ef3931.testnet` (earlier development
deployments used `agent-guard-*` account IDs).

> A NEAR function-call key restricts *which method* an agent may call. It does not
> bind the payment's recipient and amount. LatchGuard requires the agent to sign an
> intent containing `(recipient, amount, nonce, valid_until)`. The contract verifies
> that signature, rejects reused nonces and expired intents, and enforces the
> owner's spending policy before releasing funds.

If a relay, RPC path, or host process rewrites the recipient or amount **after
signing**, execution fails with `bad_signature`. Replayed intents fail with
`nonce_replay`; stale ones with `expired_intent`. Correctly signed payments still
have to pass the allowlist, the per-tx cap, the rolling window budget, and the
velocity limit.

## Live evidence (NEAR testnet)

Contract: `latchguard-ef3931.testnet`
Owner: `agent-guard-owner-e714a8.testnet`
Contract account keys: **0** (deployer key deleted; no silent upgrade or drain)
Policy: per-tx 0.50 NEAR, rolling 60s window cap 1.00 NEAR, interval 0, timelock 30s.

| Attempt | Result | Transaction |
|---|---|---|
| Allowlist add (timelocked) | pending, then applied after 30s | `2kDZwP3bdbRiBkE86JBfztmGLFdu2zRSDExaj2QaXZCg`, `Cf4tuBkUSgRUDVg4XgmeJVRwQydNW2u3Wcykv86NRZ8j` |
| **Under-cap 0.40 NEAR to non-allowlisted attacker** | `recipient_not_allowlisted` | `4dwtBu3dGfEPy44ktHPYEUj4M4xvpKFpByEhu6hRywjC` |
| Signed 0.40 NEAR, nonce=2 | settled | `HLvBFXGsV8Nai2b8h2xFGofBcUBZTvXPbThDtxVcmPQD` |
| Signed 0.40 NEAR, nonce=3 | settled | `Hfk9k2bm2QaVinL6u1EBvJWoF3hFgjiTcbsakZVRafvL` |
| Third 0.40 NEAR inside 60s | `daily_cap_exceeded` | `BNnz7zdsdXKanoNV66EVv1hN6EYSEvYrRZZSyQKossJo` |
| 0.40 NEAR after the window | settled | `KjcJfag7pgp7648A6fdTGZ4rS8qsaYJc2QgiNGYqkVt` |
| Signature for 0.40, call asks 0.50 | `bad_signature` | `APVTkguTXnSvhxZR17h6ZcwGyJcWptRk8sNL26UsFcGi` |
| Replay nonce=2 | `nonce_replay` | `BJSNeSYxgbjp5VqD8wvZ9tKVDjeuawg6gC4JNrvTNVPp` |
| Expired intent | `expired_intent` | `B8MDtj9uNmUPGipCoY4y3j6HEfitYg9LhvWV2cHdsP1c` |
| Overspend 0.60 NEAR | `per_tx_cap_exceeded` | `5HVneude7PHHQu1PML1qfgQwfNrTYN7Q2t1v5N641xK` |
| Owner proposes higher caps | pending, still blocked | `DMZsDsVB5WSqbhNuVofte5ZjDkSt7QZcfc2Hty3Cdq8t`, `8Y9jENMfsjw5vGaXyS3bQmWwjyzu32xcV856mod3zgqH` |
| Policy applied after 30s | 0.70 NEAR settles | `A7dj5BXvxsb2A6y5JZKTKi1v18TXmNEkncA9TUKFbJgE`, `BBSSm75x7sWuXbs6keQpLmyWPYaFanSspFjjgtAGpXcE` |
| Deployer key deleted | RPC key list empty | `2sjg9FwuQMFuZgifevAKQRArD8VyXihM9rMsrgqL6wm2` |
| Owner pause / unpause (keyless contract) | works | `5ZbuuXcpneQ7b6hdF3qBi4mGPzNoMXYoP9imVu6ee9PY`, `83A2X9jC8WNiKPbUsGku6qToD4rCUtK41pg9Y2iY1StC` |

Explorer: `https://explorer.testnet.near.org/transactions/<hash>`

Read-only check: `bash scripts/verify-testnet.sh` (RPC key list, policy, spend, agent key).

## Why a contract, not just a function-call key?

| Capability | Function-call access key | Safe allowance / 4337 session key | LatchGuard |
|---|---|---|---|
| Restrict contract method | yes | n/a | yes (only `pay`/`pay_signed`) |
| Restrict recipients | no | yes | yes |
| Rolling window budget + velocity | no | partial | yes, configurable window |
| Bind recipient + amount to agent signature | no | delegate-signed message | **yes, with expiry + nonce** |
| Stop host-side rewrite after signing | no | partial | **yes** |
| Timelocked loosening | n/a | varies | **yes** |
| Agent needs NEAR for gas | yes | varies | **no** — any relayer submits `pay_signed`; the signature authorizes it |

Gasless agents: `pay_signed` does not gate `predecessor_account_id`; the agent's
signature is the authorization, so an agent without a funded NEAR account can
still pay. Relayer censorship is a liveness assumption, not a safety one.

## Policy

| Control | Meaning |
|---|---|
| Signed intent | exact `(recipient, amount, nonce, valid_until)` signed by the agent |
| Replay protection | nonces single-use on-chain; `suggested_nonce()` is advisory |
| Expiry | `valid_until` bounds signature lifetime |
| Allowlist | recipient allowlist; additions timelocked |
| Per-transaction cap | max for one payment |
| Rolling window | sum over the trailing `window_secs` (60s in the demo, 24h in prod) |
| Velocity | minimum seconds between payments |
| Timelocked loosening | raising caps/widening window/adding recipients waits `delay_secs`; tightening and pause are instant |
| Balance check | never attempts to send more than it holds |

## Threat model

**In scope**

- Host/relayer rewrites recipient or amount after signing.
- Replay of an approved intent.
- Delayed execution of an old signature.
- Prompt-injected agent choosing an out-of-policy payment: blocked by allowlist,
  per-tx cap, rolling budget or velocity.
- Compromised agent key: loss bounded by policy; owner can pause instantly;
  loosening is timelocked.

**Out of scope (stated, not implied)**

- An agent signing a malicious **in-policy** payment to an allowlisted recipient.
  LatchGuard enforces spending policy, not intent legitimacy. Recipient
  reputation/allowlist curation is future work.
- Exfiltrated signing key combined with in-policy payments: bounded per window,
  not prevented.
- Owner key compromise before a timelock matures; owner is a multisig/hardware
  account in production.
- Liveness: the host may refuse to submit.
- Failed transfers: `Promise::transfer` to an invalid recipient fails after the
  call commits; the budget/nonce are consumed. Retry with a new nonce.

## Prompt-injection harness

`cargo run --example agent_injection` deterministically replays the post-injection
tool-call state of an agent reading a poisoned invoice, with **no external API
key**: attacker address (blocked), overspend (blocked), legitimate invoice
(settles), replay (blocked). The harness is LLM-agnostic by design: LatchGuard sits
beneath any agent runtime as the signing/settlement boundary. It does not claim to
solve prompt injection upstream.

## Layout

```
src/core.rs          pure policy engine (rolling window, shared by contract/tests)
src/intent.rs        intent message + ed25519 verification (env on wasm)
src/lib.rs           near-sdk contract: state, owner controls, pay/pay_signed, views
examples/agent_demo.rs        offline policy + signature demo
examples/agent_injection.rs   deterministic injection harness
examples/agent_sign.rs        prints a key + intent signature (random or AGENT_SEED)
tests/fuzz.rs        property tests: policy invariants + signature mutations
scripts/verify-testnet.sh     read-only verification (no credentials)
scripts/verify-testnet.sh     read-only verification of the deployed state
```

## Quickstart

```bash
cargo test                        # 29 tests (27 unit + 2 property)
cargo run --example agent_demo
cargo run --example agent_injection
cargo build --release --target wasm32-unknown-unknown
```

No fixed private key is committed. `agent_sign` generates a random key unless
`AGENT_SEED` is set; example tests use a disposable constant key with no funds.

## Contract interface

```text
new(owner, per_tx_cap, daily_cap, min_interval_secs, window_secs, delay_secs)
set_policy(per_tx_cap, daily_cap, min_interval_secs, window_secs)   owner; timelocked if loosening
apply_pending_policy()                                               anyone after delay
cancel_pending_policy()                                              owner
set_allowlist(account, allowed)                                      removal instant, addition timelocked
apply_pending_allowlist()                                            anyone after delay
set_agent_key(public_key)                                            owner
pause() / unpause()                                                  owner, instant
deposit()                                                            payable
pay(recipient, amount, memo)                                         policy-checked
pay_signed(recipient, amount, nonce, valid_until, signature)         signature + policy
can_pay(recipient, amount) -> (bool, reason)                         advisory dry run
get_policy / get_allowlist / get_spent_in_window / is_paused / owner
agent_key / nonce_used / suggested_nonce / pending_policy / pending_allowlist
window_secs / delay_secs
```

## Deployment notes

The contract account's deployer key is deleted after deployment (RPC shows 0
keys). Consequence: **the contract is immutable** — a bug cannot be patched in
place, and there is no admin withdrawal. Recovery means deploying a new contract
and migrating balances; this is deliberate for the hackathon. The separate owner
account keeps policy/pause authority.

## Limitations / roadmap

- Native NEAR only; NEP-141 and NEAR Intents next.
- Spend log is a bounded `Vec` (2048 entries, pruned by window); production would
  use a ring buffer.
- No owner approval queue for over-cap payments (binary allow/block today).
- No SDK yet; `agent_sign` documents the message format.
- Not audited; testnet only.

## License

MIT.
