# NEAR Hackathon submission kit — LatchGuard

## Form fields

- **Project name:** LatchGuard
- **Team lead:** Rahaman
- **Email:** rhmnac@gmail.com
- **Team size:** 1 — Solo builder
- **Tracks:** 06 — AI Security & Defense
- **NEAR wallet address:** _fill in your mainnet wallet_
- **Repository link:** _public GitHub URL_
- **Demo post link:** _public X post URL_
- **What did you build?** (20–2,000 chars):

```
LatchGuard binds NEAR agent payment intents to the recipient, amount, nonce, and expiry using
ed25519 signatures. A keyless contract enforces spending policy and owner controls on-chain,
rejecting post-signature tampering, replay, expiry, and overspending. The agent never holds
funds and needs no NEAR for gas: any relayer can submit the signed intent, and the signature
authorizes it. Owner loosening (higher caps, wider window, new recipients) is timelocked;
tightening and pause are instant. Verified live on testnet: an under-cap 0.40 NEAR payment to a
non-allowlisted attacker was rejected; three 0.40 NEAR payments hit the rolling window cap and
one settled again after 60s; tampered, replayed, expired and overspending intents were all
blocked; owner paused the keyless contract. 38 tests include property tests for policy
invariants and signature mutation. The contract account has 0 keys (deployer key deleted).
LatchGuard protects intent integrity and spending policy; it does not claim to establish intent
legitimacy or protect an exfiltrated signing key.
```

## X demo post (draft)

```
Built LatchGuard at @NEARProtocol #NEARHackathon

AI agents get money without the keys. The agent signs (recipient, amount, nonce, expiry); a
keyless NEAR contract enforces allowlist + per-tx + rolling-window caps + velocity.

Live testnet:
- under-cap payment to attacker -> blocked
- tampered amount -> bad_signature
- replay -> nonce_replay
- third payment in 60s -> daily_cap_exceeded, settles again after the window
- owner loosening timelocked; contract has 0 keys

Repo: <link>  Demo: <link>
#NEARHackathon
```

## Pre-submit checklist

- [ ] Repository is public (judges cannot open private repos).
- [ ] `cargo test` passes from a clean clone (38 tests).
- [ ] `bash scripts/verify-testnet.sh` shows `key_count=0`.
- [ ] X post published, tags @NEARProtocol and #NEARHackathon, includes video or screenshots.
- [ ] Form submitted before 14 Nov 2026 23:59 UTC with both links and a mainnet wallet.
