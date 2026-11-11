//! LatchGuard — signature-bound, policy-enforced payments for AI agents on NEAR.
//!
//! Funds live in the contract. Agents can only move them through `pay` or
//! `pay_signed`; both delegate to the shared policy engine (`core::evaluate`).
//! Signed payments bind recipient, amount, nonce and expiry to the agent's
//! ed25519 key. Owner loosening (higher caps, wider window, new recipients) is
//! timelocked; tightening and pause are instant.

use near_sdk::{env, json_types::U128, near, AccountId, NearToken, PanicOnDefault, Promise};

pub mod core;
pub mod intent;

use crate::core::{evaluate, outflow_rate_le, prune, spent_in_window, Policy};

/// Hard bound on the rolling-window log. Velocity limits already bound growth;
/// this prevents pathological spam from making `pay` unusable.
const MAX_SPENDS: usize = 2_048;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct LatchGuard {
    owner: AccountId,
    paused: bool,
    allowlist: Vec<AccountId>,
    per_tx_cap: u128,
    daily_cap: u128,
    min_interval_secs: u64,
    window_secs: u64,
    /// Delay before loosening changes take effect. 0 disables the timelock.
    delay_secs: u64,
    last_payment_ts: u64,
    /// Rolling (unix seconds, yoctoNEAR) payment log, trimmed to `window_secs`.
    spends: Vec<(u64, u128)>,
    /// Pending loosening: (per_tx, daily, interval, window, apply_after).
    pending_policy: Option<(u128, u128, u64, u64, u64)>,
    /// Allowlist additions waiting out the timelock: (account, apply_after).
    pending_allowlist: Vec<(AccountId, u64)>,
    /// Unpause waiting out the timelock.
    pending_unpause: Option<u64>,
    agent_pubkey: Vec<u8>,
    used_nonces: Vec<u64>,
}

#[near]
impl LatchGuard {
    #[init]
    pub fn new(
        owner: AccountId,
        per_tx_cap: U128,
        daily_cap: U128,
        min_interval_secs: u64,
        window_secs: u64,
        delay_secs: u64,
    ) -> Self {
        assert!(per_tx_cap.0 > 0, "per-tx cap must be positive");
        assert!(daily_cap.0 >= per_tx_cap.0, "daily cap must be >= per-tx cap");
        assert!(window_secs > 0, "window must be positive");
        Self {
            owner,
            paused: false,
            allowlist: Vec::new(),
            per_tx_cap: per_tx_cap.0,
            daily_cap: daily_cap.0,
            min_interval_secs,
            window_secs,
            delay_secs,
            last_payment_ts: 0,
            spends: Vec::new(),
            pending_policy: None,
            pending_allowlist: Vec::new(),
            pending_unpause: None,
            agent_pubkey: Vec::new(),
            used_nonces: Vec::new(),
        }
    }

    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner,
            "only the owner can change policy"
        );
    }

    fn now_secs() -> u64 {
        env::block_timestamp() / 1_000_000_000
    }

    fn policy(&self) -> Policy {
        Policy {
            per_tx_cap: self.per_tx_cap,
            daily_cap: self.daily_cap,
            min_interval_secs: self.min_interval_secs,
            window_secs: self.window_secs,
        }
    }

    fn apply_policy_values(&mut self, per_tx: u128, daily: u128, interval: u64, window: u64) {
        self.per_tx_cap = per_tx;
        self.daily_cap = daily;
        self.min_interval_secs = interval;
        self.window_secs = window;
        env::log_str(&format!(
            "AGENT_GUARD_POLICY per_tx={} daily={} interval={} window={}",
            per_tx, daily, interval, window
        ));
    }

    fn is_tightening(&self, per_tx: u128, daily: u128, interval: u64, window: u64) -> bool {
        per_tx <= self.per_tx_cap
            && interval >= self.min_interval_secs
            && outflow_rate_le(daily, window, self.daily_cap, self.window_secs)
    }

    // ---- owner controls -------------------------------------------------

    /// Tightening changes apply immediately; loosening is queued for `delay_secs`.
    pub fn set_policy(
        &mut self,
        per_tx_cap: U128,
        daily_cap: U128,
        min_interval_secs: u64,
        window_secs: u64,
    ) {
        self.assert_owner();
        assert!(per_tx_cap.0 > 0, "per-tx cap must be positive");
        assert!(daily_cap.0 >= per_tx_cap.0, "daily cap must be >= per-tx cap");
        assert!(window_secs > 0, "window must be positive");

        if self.delay_secs == 0
            || self.is_tightening(per_tx_cap.0, daily_cap.0, min_interval_secs, window_secs)
        {
            self.pending_policy = None;
            self.apply_policy_values(
                per_tx_cap.0,
                daily_cap.0,
                min_interval_secs,
                window_secs,
            );
        } else {
            let apply_after = Self::now_secs() + self.delay_secs;
            self.pending_policy = Some((
                per_tx_cap.0,
                daily_cap.0,
                min_interval_secs,
                window_secs,
                apply_after,
            ));
            env::log_str(&format!(
                "AGENT_GUARD_POLICY_PENDING per_tx={} daily={} apply_after={}",
                per_tx_cap.0, daily_cap.0, apply_after
            ));
        }
    }

    /// Anyone can finalize a matured loosening change (keeps liveness).
    pub fn apply_pending_policy(&mut self) {
        let now = Self::now_secs();
        match self.pending_policy {
            Some((per_tx, daily, interval, window, after)) if now >= after => {
                self.pending_policy = None;
                self.apply_policy_values(per_tx, daily, interval, window);
            }
            Some((_, _, _, _, after)) => {
                env::panic_str(&format!("pending policy not ready until {}", after));
            }
            None => env::panic_str("no pending policy"),
        }
    }

    pub fn cancel_pending_policy(&mut self) {
        self.assert_owner();
        self.pending_policy = None;
        env::log_str("AGENT_GUARD_POLICY_CANCELLED");
    }

    /// Removing a recipient is instant; adding one waits out the timelock.
    pub fn set_allowlist(&mut self, account: AccountId, allowed: bool) {
        self.assert_owner();
        if !allowed {
            self.allowlist.retain(|a| a != &account);
            self.pending_allowlist.retain(|(a, _)| a != &account);
            env::log_str(&format!(
                "AGENT_GUARD_ALLOWLIST account={} allowed=false",
                account
            ));
            return;
        }
        if self.allowlist.contains(&account) {
            return;
        }
        if self.delay_secs == 0 {
            self.allowlist.push(account.clone());
            env::log_str(&format!(
                "AGENT_GUARD_ALLOWLIST account={} allowed=true",
                account
            ));
        } else {
            let after = Self::now_secs() + self.delay_secs;
            if let Some(entry) = self
                .pending_allowlist
                .iter_mut()
                .find(|(a, _)| a == &account)
            {
                entry.1 = after;
            } else {
                self.pending_allowlist.push((account.clone(), after));
            }
            env::log_str(&format!(
                "AGENT_GUARD_ALLOWLIST_PENDING account={} apply_after={}",
                account, after
            ));
        }
    }

    /// Anyone can finalize matured allowlist additions.
    pub fn apply_pending_allowlist(&mut self) {
        let now = Self::now_secs();
        let mut still_pending = Vec::new();
        for (account, after) in std::mem::take(&mut self.pending_allowlist) {
            if now >= after {
                if !self.allowlist.contains(&account) {
                    self.allowlist.push(account.clone());
                }
                env::log_str(&format!(
                    "AGENT_GUARD_ALLOWLIST account={} allowed=true",
                    account
                ));
            } else {
                still_pending.push((account, after));
            }
        }
        self.pending_allowlist = still_pending;
    }

    pub fn set_agent_key(&mut self, public_key: Vec<u8>) {
        self.assert_owner();
        assert_eq!(public_key.len(), 32, "ed25519 public key must be 32 bytes");
        self.agent_pubkey = public_key;
        env::log_str("AGENT_GUARD_AGENT_KEY_SET");
    }

    pub fn pause(&mut self) {
        self.assert_owner();
        self.paused = true;
        self.pending_unpause = None;
        env::log_str("AGENT_GUARD_PAUSED");
    }

    pub fn unpause(&mut self) {
        self.assert_owner();
        if self.delay_secs == 0 {
            self.paused = false;
            self.pending_unpause = None;
            env::log_str("AGENT_GUARD_UNPAUSED");
        } else {
            let after = Self::now_secs() + self.delay_secs;
            self.pending_unpause = Some(after);
            env::log_str(&format!("AGENT_GUARD_UNPAUSE_PENDING apply_after={}", after));
        }
    }

    /// Anyone can finalize a matured unpause.
    pub fn apply_unpause(&mut self) {
        let now = Self::now_secs();
        match self.pending_unpause {
            Some(after) if now >= after => {
                self.paused = false;
                self.pending_unpause = None;
                env::log_str("AGENT_GUARD_UNPAUSED");
            }
            Some(after) => env::panic_str(&format!("unpause not ready until {}", after)),
            None => env::panic_str("no pending unpause"),
        }
    }

    // ---- funds ----------------------------------------------------------

    #[payable]
    pub fn deposit(&mut self) {
        env::log_str(&format!(
            "AGENT_GUARD_DEPOSIT amount={} balance={}",
            env::attached_deposit().as_yoctonear(),
            env::account_balance().as_yoctonear()
        ));
    }

    pub fn pay(&mut self, recipient: AccountId, amount: U128, memo: String) -> Promise {
        self.execute_payment(recipient, amount.0, memo)
    }

    /// Policy-checked transfer bound to a signed, expiring intent.
    /// The relayer may be anyone; the signature authorizes the payment.
    pub fn pay_signed(
        &mut self,
        recipient: AccountId,
        amount: U128,
        nonce: u64,
        valid_until: u64,
        signature: Vec<u8>,
    ) -> Promise {
        let now = Self::now_secs();
        if self.agent_pubkey.is_empty() {
            env::log_str("AGENT_GUARD_BLOCK reason=agent_key_missing");
            env::panic_str("blocked: agent_key_missing");
        }
        if now > valid_until {
            env::log_str(&format!(
                "AGENT_GUARD_BLOCK recipient={} nonce={} reason=expired_intent",
                recipient, nonce
            ));
            env::panic_str("blocked: expired_intent");
        }
        if self.used_nonces.contains(&nonce) {
            env::log_str(&format!(
                "AGENT_GUARD_BLOCK recipient={} nonce={} reason=nonce_replay",
                recipient, nonce
            ));
            env::panic_str("blocked: nonce_replay");
        }
        let message = intent::message(
            env::current_account_id().as_str(),
            recipient.as_str(),
            amount.0,
            nonce,
            valid_until,
        );
        if !intent::verify(&self.agent_pubkey, &message, &signature) {
            env::log_str(&format!(
                "AGENT_GUARD_BLOCK recipient={} amount={} nonce={} reason=bad_signature",
                recipient, amount.0, nonce
            ));
            env::panic_str("blocked: bad_signature");
        }
        self.used_nonces.push(nonce);

        let memo = format!("signed intent nonce={}", nonce);
        self.execute_payment(recipient, amount.0, memo)
    }

    fn execute_payment(&mut self, recipient: AccountId, amount: u128, memo: String) -> Promise {
        let now = Self::now_secs();
        prune(&mut self.spends, now, self.window_secs);
        if self.spends.len() >= MAX_SPENDS {
            env::log_str(&format!(
                "AGENT_GUARD_BLOCK recipient={} amount={} reason=spend_log_full",
                recipient, amount
            ));
            env::panic_str("blocked: spend_log_full");
        }
        let spent = spent_in_window(&self.spends, now, self.window_secs);

        let allowlisted = self.allowlist.contains(&recipient);
        match evaluate(
            self.policy(),
            self.paused,
            allowlisted,
            spent,
            amount,
            now,
            self.last_payment_ts,
            env::account_balance().as_yoctonear(),
        ) {
            Ok(()) => {
                self.spends.push((now, amount));
                self.last_payment_ts = now;
                env::log_str(&format!(
                    "AGENT_GUARD_ALLOW recipient={} amount={} memo={}",
                    recipient, amount, memo
                ));
                Promise::new(recipient).transfer(NearToken::from_yoctonear(amount))
            }
            Err(reason) => {
                env::log_str(&format!(
                    "AGENT_GUARD_BLOCK recipient={} amount={} reason={}",
                    recipient,
                    amount,
                    reason.as_str()
                ));
                env::panic_str(&format!("blocked: {}", reason.as_str()))
            }
        }
    }

    // ---- views ----------------------------------------------------------

    pub fn can_pay(&self, recipient: AccountId, amount: U128) -> (bool, String) {
        let now = Self::now_secs();
        let mut spends = self.spends.clone();
        prune(&mut spends, now, self.window_secs);
        let spent = spent_in_window(&spends, now, self.window_secs);
        let allowlisted = self.allowlist.contains(&recipient);
        match evaluate(
            self.policy(),
            self.paused,
            allowlisted,
            spent,
            amount.0,
            now,
            self.last_payment_ts,
            env::account_balance().as_yoctonear(),
        ) {
            Ok(()) => (true, "ok".to_string()),
            Err(reason) => (false, reason.as_str().to_string()),
        }
    }

    pub fn get_policy(&self) -> (U128, U128, u64, u64) {
        (
            U128(self.per_tx_cap),
            U128(self.daily_cap),
            self.min_interval_secs,
            self.window_secs,
        )
    }

    pub fn get_allowlist(&self) -> Vec<AccountId> {
        self.allowlist.clone()
    }

    pub fn get_spent_in_window(&self) -> U128 {
        let now = Self::now_secs();
        let mut spends = self.spends.clone();
        prune(&mut spends, now, self.window_secs);
        U128(spent_in_window(&spends, now, self.window_secs))
    }

    pub fn pending_policy(&self) -> Option<(U128, U128, u64, u64, u64)> {
        self.pending_policy
            .map(|(a, b, c, d, e)| (U128(a), U128(b), c, d, e))
    }

    pub fn pending_allowlist(&self) -> Vec<(AccountId, u64)> {
        self.pending_allowlist.clone()
    }

    pub fn pending_unpause(&self) -> Option<u64> {
        self.pending_unpause
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn owner(&self) -> AccountId {
        self.owner.clone()
    }

    pub fn agent_key(&self) -> Vec<u8> {
        self.agent_pubkey.clone()
    }

    pub fn nonce_used(&self, nonce: u64) -> bool {
        self.used_nonces.contains(&nonce)
    }

    pub fn suggested_nonce(&self) -> u64 {
        self.used_nonces
            .iter()
            .copied()
            .max()
            .map(|n| n.saturating_add(1))
            .unwrap_or(0)
    }

    pub fn window_secs(&self) -> u64 {
        self.window_secs
    }

    pub fn delay_secs(&self) -> u64 {
        self.delay_secs
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    const NEAR: u128 = 1_000_000_000_000_000_000_000_000;

    fn context(predecessor: AccountId, ts_secs: u64) -> VMContextBuilder {
        let mut b = VMContextBuilder::new();
        b.current_account_id("agent-guard.testnet".parse().unwrap())
            .signer_account_id(predecessor.clone())
            .predecessor_account_id(predecessor)
            .block_timestamp(ts_secs * 1_000_000_000)
            .account_balance(NearToken::from_yoctonear(10 * NEAR));
        b
    }

    fn contract() -> LatchGuard {
        testing_env!(context(accounts(0), 1_700_000_100).build());
        LatchGuard::new(accounts(0), U128(NEAR), U128(2 * NEAR), 0, 60, 0)
    }

    fn delayed_contract() -> LatchGuard {
        testing_env!(context(accounts(0), 1_700_000_100).build());
        LatchGuard::new(accounts(0), U128(NEAR), U128(2 * NEAR), 0, 3_600, 30)
    }

    fn sign(recipient: &str, amount: u128, nonce: u64, valid_until: u64) -> Vec<u8> {
        use ed25519_dalek::{Signer, SigningKey};
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let msg = intent::message("agent-guard.testnet", recipient, amount, nonce, valid_until);
        key.sign(&msg).to_bytes().to_vec()
    }

    fn register_key(c: &mut LatchGuard) {
        use ed25519_dalek::SigningKey;
        let key = SigningKey::from_bytes(&[7u8; 32]);
        c.set_agent_key(key.verifying_key().to_bytes().to_vec());
    }

    #[test]
    fn pay_allowed_updates_state() {
        let mut c = contract();
        c.set_allowlist(accounts(1), true);
        testing_env!(context(accounts(1), 1_700_000_100).build());
        let _ = c.pay(accounts(1), U128(NEAR / 2), "ok".into());
        assert_eq!(c.get_spent_in_window().0, NEAR / 2);
    }

    #[test]
    #[should_panic(expected = "blocked: recipient_not_allowlisted")]
    fn pay_blocks_unknown_recipient() {
        let mut c = contract();
        let _ = c.pay(accounts(1), U128(NEAR / 2), "no".into());
    }

    #[test]
    #[should_panic(expected = "blocked: per_tx_cap_exceeded")]
    fn pay_blocks_over_cap() {
        let mut c = contract();
        c.set_allowlist(accounts(1), true);
        let _ = c.pay(accounts(1), U128(NEAR + 1), "no".into());
    }

    #[test]
    #[should_panic(expected = "only the owner can change policy")]
    fn non_owner_cannot_change_policy() {
        let mut c = contract();
        testing_env!(context(accounts(1), 1_700_000_100).build());
        c.pause();
    }

    #[test]
    fn pay_signed_accepts_valid_intent() {
        let mut c = contract();
        register_key(&mut c);
        c.set_allowlist(accounts(1), true);

        let amount = NEAR / 2;
        let nonce = 1u64;
        let until = 1_700_000_200u64;
        let sig = sign(accounts(1).as_str(), amount, nonce, until);

        testing_env!(context(accounts(1), 1_700_000_100).build());
        let _ = c.pay_signed(accounts(1), U128(amount), nonce, until, sig);
        assert!(c.nonce_used(1));
        assert_eq!(c.suggested_nonce(), 2);
    }

    #[test]
    #[should_panic(expected = "blocked: nonce_replay")]
    fn pay_signed_rejects_replay() {
        let mut c = contract();
        register_key(&mut c);
        c.set_allowlist(accounts(1), true);

        let amount = NEAR / 2;
        let nonce = 7u64;
        let until = 1_700_000_200u64;
        let sig = sign(accounts(1).as_str(), amount, nonce, until);

        testing_env!(context(accounts(1), 1_700_000_100).build());
        let _ = c.pay_signed(accounts(1), U128(amount), nonce, until, sig.clone());
        let _ = c.pay_signed(accounts(1), U128(amount), nonce, until, sig);
    }

    #[test]
    #[should_panic(expected = "blocked: bad_signature")]
    fn pay_signed_rejects_tampered_recipient() {
        let mut c = contract();
        register_key(&mut c);
        c.set_allowlist(accounts(1), true);
        c.set_allowlist(accounts(2), true);

        let amount = NEAR / 2;
        let nonce = 3u64;
        let until = 1_700_000_200u64;
        let sig = sign(accounts(2).as_str(), amount, nonce, until);

        testing_env!(context(accounts(1), 1_700_000_100).build());
        let _ = c.pay_signed(accounts(1), U128(amount), nonce, until, sig);
    }

    #[test]
    #[should_panic(expected = "blocked: expired_intent")]
    fn pay_signed_rejects_expired_intent() {
        let mut c = contract();
        register_key(&mut c);
        c.set_allowlist(accounts(1), true);

        let amount = NEAR / 2;
        let nonce = 9u64;
        let until = 1_700_000_050u64;
        let sig = sign(accounts(1).as_str(), amount, nonce, until);

        testing_env!(context(accounts(1), 1_700_000_100).build());
        let _ = c.pay_signed(accounts(1), U128(amount), nonce, until, sig);
    }

    #[test]
    fn rolling_window_frees_budget() {
        let mut c = contract();
        c.set_allowlist(accounts(1), true);

        testing_env!(context(accounts(1), 1_700_000_100).build());
        let _ = c.pay(accounts(1), U128(NEAR), "a".into());
        testing_env!(context(accounts(1), 1_700_000_110).build());
        let _ = c.pay(accounts(1), U128(NEAR), "b".into());
        // 2 NEAR spent inside the 60s window; daily cap is 2 NEAR.
        testing_env!(context(accounts(1), 1_700_000_120).build());
        assert_eq!(c.can_pay(accounts(1), U128(NEAR)).0, false);
        // 61s after the first payment, its budget returns.
        testing_env!(context(accounts(1), 1_700_000_161).build());
        assert_eq!(c.can_pay(accounts(1), U128(NEAR)).0, true);
    }

    #[test]
    fn loosening_is_timelocked() {
        let mut c = delayed_contract();
        c.set_allowlist(accounts(1), true);

        c.set_policy(U128(5 * NEAR), U128(10 * NEAR), 0, 3_600);
        assert!(c.pending_policy().is_some());
        assert_eq!(c.can_pay(accounts(1), U128(5 * NEAR)).0, false);

        testing_env!(context(accounts(1), 1_700_000_131).build());
        c.apply_pending_policy();
        c.apply_pending_allowlist();
        assert!(c.pending_policy().is_none());
        assert_eq!(c.can_pay(accounts(1), U128(5 * NEAR)).0, true);
    }

    #[test]
    fn tightening_is_immediate() {
        let mut c = contract();
        c.set_policy(U128(NEAR / 2), U128(NEAR), 30, 60);
        assert!(c.pending_policy().is_none());
        assert_eq!(c.get_policy().0 .0, NEAR / 2);
    }

    #[test]
    fn allowlist_addition_is_timelocked() {
        let mut c = delayed_contract();
        c.set_allowlist(accounts(1), true);
        assert!(!c.get_allowlist().contains(&accounts(1)));
        assert_eq!(c.pending_allowlist().len(), 1);

        testing_env!(context(accounts(1), 1_700_000_131).build());
        c.apply_pending_allowlist();
        assert!(c.get_allowlist().contains(&accounts(1)));
    }

    #[test]
    fn narrowing_window_is_timelocked() {
        // Same cap over a narrower window permits more per second: loosening.
        let mut c = delayed_contract();
        c.set_policy(U128(NEAR), U128(2 * NEAR), 0, 30);
        assert!(c.pending_policy().is_some());
        assert_eq!(c.window_secs(), 3_600);
    }

    #[test]
    fn widening_window_is_immediate() {
        let mut c = delayed_contract();
        c.set_policy(U128(NEAR), U128(2 * NEAR), 0, 7_200);
        assert!(c.pending_policy().is_none());
        assert_eq!(c.window_secs(), 7_200);
    }

    #[test]
    fn tightening_clears_pending_loosening() {
        let mut c = delayed_contract();
        c.set_policy(U128(5 * NEAR), U128(10 * NEAR), 0, 3_600);
        assert!(c.pending_policy().is_some());
        c.set_policy(U128(NEAR / 2), U128(NEAR), 0, 3_600);
        assert!(c.pending_policy().is_none());
        assert_eq!(c.get_policy().0 .0, NEAR / 2);
    }

    #[test]
    fn unpause_is_timelocked() {
        let mut c = delayed_contract();
        c.pause();
        assert!(c.is_paused());

        c.unpause();
        assert!(c.is_paused(), "unpause must wait for the timelock");
        assert!(c.pending_unpause().is_some());

        testing_env!(context(accounts(0), 1_700_000_131).build());
        c.apply_unpause();
        assert!(!c.is_paused());
    }
}
