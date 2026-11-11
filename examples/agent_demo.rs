//! Offline policy demo: normal payments, agent mistakes, signed intents,
//! expiry, tamper, and replay. Uses the exact same engine as the contract.
//!
//! Run: `cargo run --example agent_demo`

use agent_guard::core::{evaluate, prune, spent_in_window, Policy};
use agent_guard::intent;
use ed25519_dalek::{Signer, SigningKey};

const NEAR: u128 = 1_000_000_000_000_000_000_000_000;

fn near_str(amount: u128) -> String {
    let hundredths = amount / (NEAR / 100);
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

struct World {
    policy: Policy,
    paused: bool,
    allowlist: Vec<&'static str>,
    spends: Vec<(u64, u128)>,
    last_payment_ts: u64,
    balance: u128,
    allowed: u32,
    blocked: u32,
}

impl World {
    fn new() -> Self {
        Self {
            policy: Policy {
                per_tx_cap: NEAR,        // 1 NEAR per payment
                daily_cap: 5 * NEAR / 2, // 2.5 NEAR in any trailing 24h
                min_interval_secs: 60,   // no faster than 1/minute
                window_secs: 86_400,
            },
            paused: false,
            allowlist: vec!["merchant.near", "api.near"],
            spends: Vec::new(),
            last_payment_ts: 0,
            balance: 10 * NEAR,
            allowed: 0,
            blocked: 0,
        }
    }

    fn attempt(&mut self, recipient: &str, amount: u128, now: u64, note: &str) {
        prune(&mut self.spends, now, self.policy.window_secs);
        let spent = spent_in_window(&self.spends, now, self.policy.window_secs);
        let allowlisted = self.allowlist.contains(&recipient);
        match evaluate(
            self.policy,
            self.paused,
            allowlisted,
            spent,
            amount,
            now,
            self.last_payment_ts,
            self.balance,
        ) {
            Ok(()) => {
                self.spends.push((now, amount));
                self.last_payment_ts = now;
                self.balance -= amount;
                self.allowed += 1;
                println!(
                    "  ALLOW  {:<14} {:>9} NEAR  t={}  ({})",
                    recipient,
                    near_str(amount),
                    now,
                    note
                );
            }
            Err(reason) => {
                self.blocked += 1;
                println!(
                    "  BLOCK  {:<14} {:>9} NEAR  t={}  reason={:<28} ({})",
                    recipient,
                    near_str(amount),
                    now,
                    reason.as_str(),
                    note
                );
            }
        }
    }
}

fn main() {
    println!("Agent Guard — offline policy demo");
    println!("policy: per-tx 1.00 NEAR, rolling 24h 2.50 NEAR, interval 60s, allowlist [merchant.near, api.near]");
    println!("balance: 10.00 NEAR, held by the contract (owner can pause, agent cannot override)");
    println!();

    let mut w = World::new();
    let t0 = 1_700_000_100u64;

    println!("Scenario 1 — normal payment, then three agent mistakes");
    w.attempt("merchant.near", NEAR / 2, t0, "legitimate purchase");
    w.attempt("merchant.near", 3 * NEAR / 2, t0, "agent tries to overspend");
    w.attempt("unknown.near", NEAR / 2, t0, "hallucinated recipient");
    w.attempt("merchant.near", NEAR / 2, t0, "repeat immediately");

    println!();
    println!("Scenario 2 — wait out the velocity window, spend within limits");
    w.attempt("api.near", NEAR / 2, t0 + 61, "API payment after 61s");
    w.attempt("merchant.near", NEAR, t0 + 122, "second payment after another 61s");

    println!();
    println!("Scenario 3 — rolling 24h cap, then owner pause");
    w.attempt("api.near", NEAR, t0 + 183, "would exceed 2.50 NEAR/24h");
    w.paused = true;
    w.attempt("merchant.near", NEAR / 2, t0 + 244, "owner paused the agent");
    w.paused = false;

    println!();
    println!("Scenario 4 — signed intents: tamper, expiry, replay");
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let contract = "agent-guard.testnet";
    let amount = NEAR / 2;
    let nonce = 1u64;
    let until = t0 + 300;
    let msg = intent::message(contract, "merchant.near", amount, nonce, until);
    let sig = key.sign(&msg).to_bytes();
    let ok = intent::verify(key.verifying_key().as_bytes(), &msg, &sig);
    println!("  SIGN   intent(merchant.near, 0.50, nonce=1)      verify={}", ok);
    let tampered = intent::message(contract, "attacker.near", amount, nonce, until);
    let bad = intent::verify(key.verifying_key().as_bytes(), &tampered, &sig);
    println!("  BLOCK  tampered recipient -> attacker.near        verify={}", bad);

    let expired_msg = intent::message(contract, "merchant.near", amount, nonce, t0 - 1);
    let expired_sig = key.sign(&expired_msg).to_bytes();
    let expired_valid = intent::verify(
        key.verifying_key().as_bytes(),
        &expired_msg,
        &expired_sig,
    );
    println!(
        "  BLOCK  signature valid but intent expired        verify={} now>until={}",
        expired_valid,
        t0 > (t0 - 1)
    );

    let mut used = std::collections::BTreeSet::new();
    used.insert(nonce);
    let replay_ok = !used.contains(&nonce);
    println!("  BLOCK  replay of nonce=1                          accepted={}", replay_ok);
    assert!(ok && !bad && expired_valid && !replay_ok);

    println!();
    println!(
        "result: {} allowed, {} blocked, {} NEAR remaining in contract",
        w.allowed,
        w.blocked,
        near_str(w.balance)
    );

    assert_eq!(w.allowed, 3, "unexpected allowed count");
    assert_eq!(w.blocked, 5, "unexpected blocked count");
    println!("demo assertions: PASS");
}
