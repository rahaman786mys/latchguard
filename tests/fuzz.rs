//! Property tests for the two things that must never break:
//! 1. the policy engine never allows a payment that violates its own limits;
//! 2. any mutation of an ed25519 signature is rejected.

use agent_guard::core::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn policy_never_allows_an_invariant_break(
        per_tx in 1u128..1_000_000u128,
        extra in 0u128..1_000_000u128,
        interval in 0u64..120,
        ops in prop::collection::vec((0u64..500, 1u128..2_000_000u128), 0..40),
    ) {
        let policy = Policy {
            per_tx_cap: per_tx,
            daily_cap: per_tx + extra,
            min_interval_secs: interval,
            window_secs: 86_400,
        };
        let mut spends: Vec<Spend> = Vec::new();
        let mut last = 0u64;
        let mut now = 1_700_000_000u64;

        for (dt, amount) in ops {
            now = now.saturating_add(dt);
            prune(&mut spends, now, policy.window_secs);
            let spent = spent_in_window(&spends, now, policy.window_secs);
            let res = evaluate(policy, false, true, spent, amount, now, last, u128::MAX);
            if res.is_ok() {
                prop_assert!(amount <= policy.per_tx_cap);
                prop_assert!(spent.saturating_add(amount) <= policy.daily_cap);
                prop_assert!(last == 0 || now - last >= policy.min_interval_secs);
                spends.push((now, amount));
                last = now;
            }
        }
    }
}

proptest! {
    #[test]
    fn signature_mutation_is_rejected(idx in 0usize..64, val in 0u8..=255u8) {
        use ed25519_dalek::{Signer, SigningKey};

        let key = SigningKey::from_bytes(&[7u8; 32]);
        let msg = agent_guard::intent::message("c.testnet", "r.near", 1, 1, 1_700_000_000);
        let mut sig = key.sign(&msg).to_bytes();
        if sig[idx] == val {
            sig[idx] = val.wrapping_add(1);
        } else {
            sig[idx] = val;
        }
        prop_assert!(!agent_guard::intent::verify(
            key.verifying_key().as_bytes(),
            &msg,
            &sig
        ));
    }
}
