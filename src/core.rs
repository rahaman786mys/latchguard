//! Pure spending-policy engine shared by the on-chain contract and the demos.
//!
//! No NEAR types here on purpose: the same code that enforces policy on-chain is
//! exercised natively by the examples and the property tests.

/// Caps are in yoctoNEAR (1 NEAR = 10^24 yocto).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub per_tx_cap: u128,
    pub daily_cap: u128,
    pub min_interval_secs: u64,
    /// Rolling window in seconds (production: 86400; demo instance: 60).
    pub window_secs: u64,
}

/// One completed payment: (unix seconds, amount in yocto).
pub type Spend = (u64, u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny {
    Paused,
    ZeroAmount,
    NotAllowlisted,
    PerTxCap,
    DailyCap,
    Velocity,
    InsufficientBalance,
}

impl Deny {
    pub fn as_str(self) -> &'static str {
        match self {
            Deny::Paused => "paused",
            Deny::ZeroAmount => "zero_amount",
            Deny::NotAllowlisted => "recipient_not_allowlisted",
            Deny::PerTxCap => "per_tx_cap_exceeded",
            Deny::DailyCap => "window_cap_exceeded",
            Deny::Velocity => "velocity_limit",
            Deny::InsufficientBalance => "insufficient_balance",
        }
    }
}

/// True when `(daily_a / window_a) <= (daily_b / window_b)`, i.e. policy A can
/// never move more per second than policy B. Used to decide whether a policy
/// change is tightening (instant) or loosening (timelocked). Cross-multiplied to
/// avoid floats; values fit comfortably in u128.
pub fn outflow_rate_le(daily_a: u128, window_a: u64, daily_b: u128, window_b: u64) -> bool {
    assert!(window_a > 0 && window_b > 0, "window must be positive");
    daily_a.saturating_mul(window_b as u128) <= daily_b.saturating_mul(window_a as u128)
}

/// Drop payments that fell out of the trailing window.
pub fn prune(spends: &mut Vec<Spend>, now: u64, window_secs: u64) {
    let cutoff = now.saturating_sub(window_secs);
    spends.retain(|(at, _)| *at > cutoff);
}

/// Sum of payments inside the trailing window.
pub fn spent_in_window(spends: &[Spend], now: u64, window_secs: u64) -> u128 {
    let cutoff = now.saturating_sub(window_secs);
    spends
        .iter()
        .filter(|(at, _)| *at > cutoff)
        .map(|(_, amount)| *amount)
        .sum()
}

/// The single decision point. The contract must not transfer unless this returns `Ok`.
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    policy: Policy,
    paused: bool,
    allowlisted: bool,
    spent_in_window: u128,
    amount: u128,
    now: u64,
    last_payment_ts: u64,
    balance: u128,
) -> Result<(), Deny> {
    if paused {
        return Err(Deny::Paused);
    }
    if amount == 0 {
        return Err(Deny::ZeroAmount);
    }
    if !allowlisted {
        return Err(Deny::NotAllowlisted);
    }
    if amount > policy.per_tx_cap {
        return Err(Deny::PerTxCap);
    }
    if spent_in_window.saturating_add(amount) > policy.daily_cap {
        return Err(Deny::DailyCap);
    }
    if last_payment_ts != 0
        && now.saturating_sub(last_payment_ts) < policy.min_interval_secs
    {
        return Err(Deny::Velocity);
    }
    if balance < amount {
        return Err(Deny::InsufficientBalance);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEAR: u128 = 1_000_000_000_000_000_000_000_000;
    const DAY: u64 = 86_400;

    fn policy() -> Policy {
        Policy {
            per_tx_cap: NEAR,
            daily_cap: 5 * NEAR / 2,
            min_interval_secs: 60,
            window_secs: DAY,
        }
    }

    #[test]
    fn allows_payment_within_all_limits() {
        assert_eq!(
            evaluate(policy(), false, true, 0, NEAR / 2, 1_700_000_100, 0, 10 * NEAR),
            Ok(())
        );
    }

    #[test]
    fn blocks_non_allowlisted_recipient() {
        assert_eq!(
            evaluate(policy(), false, false, 0, NEAR / 2, 1_700_000_100, 0, 10 * NEAR),
            Err(Deny::NotAllowlisted)
        );
    }

    #[test]
    fn blocks_over_per_tx_cap() {
        assert_eq!(
            evaluate(policy(), false, true, 0, NEAR + 1, 1_700_000_100, 0, 10 * NEAR),
            Err(Deny::PerTxCap)
        );
    }

    #[test]
    fn blocks_over_daily_cap() {
        assert_eq!(
            evaluate(policy(), false, true, 2 * NEAR, NEAR, 1_700_000_100, 0, 10 * NEAR),
            Err(Deny::DailyCap)
        );
    }

    #[test]
    fn blocks_velocity_repeat() {
        assert_eq!(
            evaluate(policy(), false, true, 0, NEAR / 2, 1_700_000_130, 1_700_000_100, 10 * NEAR),
            Err(Deny::Velocity)
        );
    }

    #[test]
    fn allows_velocity_after_interval() {
        assert_eq!(
            evaluate(policy(), false, true, 0, NEAR / 2, 1_700_000_161, 1_700_000_100, 10 * NEAR),
            Ok(())
        );
    }

    #[test]
    fn blocks_when_paused() {
        assert_eq!(
            evaluate(policy(), true, true, 0, NEAR / 2, 1_700_000_100, 0, 10 * NEAR),
            Err(Deny::Paused)
        );
    }

    #[test]
    fn blocks_zero_and_insufficient() {
        assert_eq!(
            evaluate(policy(), false, true, 0, 0, 1_700_000_100, 0, 10 * NEAR),
            Err(Deny::ZeroAmount)
        );
        assert_eq!(
            evaluate(policy(), false, true, 0, NEAR / 2, 1_700_000_100, 0, NEAR / 4),
            Err(Deny::InsufficientBalance)
        );
    }

    #[test]
    fn window_prunes_old_spends() {
        let now = 1_700_000_000u64;
        let mut spends: Vec<Spend> = vec![
            (now - 100_000, 5 * NEAR),
            (now - 10, NEAR),
            (now - 1, NEAR / 2),
        ];
        prune(&mut spends, now, DAY);
        assert_eq!(spends.len(), 2);
        assert_eq!(spent_in_window(&spends, now, DAY), 3 * NEAR / 2);
    }

    #[test]
    fn short_window_frees_quickly() {
        let now = 1_700_000_000u64;
        let spends: Vec<Spend> = vec![(now - 120, 3 * NEAR), (now - 30, NEAR)];
        assert_eq!(spent_in_window(&spends, now, 60), NEAR);
        assert_eq!(spent_in_window(&spends, now, DAY), 4 * NEAR);
    }

    #[test]
    fn window_boundary_is_not_double_counted() {
        let now = 1_700_000_000u64;
        let spends: Vec<Spend> = vec![(now - DAY, 5 * NEAR)];
        assert_eq!(spent_in_window(&spends, now, DAY), 0);
    }

    #[test]
    fn outflow_rate_direction_is_correct() {
        // Same cap, narrower window = more per second = loosening.
        assert!(!outflow_rate_le(NEAR, 60, NEAR, DAY));
        // Same cap, wider window = less per second = tightening.
        assert!(outflow_rate_le(NEAR, DAY, NEAR, 3600));
        // Higher cap is loosening; lower cap is tightening.
        assert!(!outflow_rate_le(2 * NEAR, DAY, NEAR, DAY));
        assert!(outflow_rate_le(NEAR, DAY, 2 * NEAR, DAY));
        // Equal rates are not loosening.
        assert!(outflow_rate_le(NEAR, 3600, 24 * NEAR, DAY));
    }
}
