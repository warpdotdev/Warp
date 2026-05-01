//! Integration tests for the [`orchestrator::budget`] module.
//!
//! These tests cover tier transitions, cap-exceeded errors, reset semantics,
//! serde round-tripping of the snapshot, and the atomicity guarantee under
//! contention.

use std::collections::HashMap;
use std::sync::Arc;

use orchestrator::{
    Budget, BudgetError, BudgetSnapshot, BudgetTier, Cap, CustomProviderId, Provider,
};

/// Compile-time assertion that [`Budget`] is `Send + Sync`. If a future
/// change introduces a non-`Send`/`Sync` field this will fail to compile.
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn budget_is_send_sync() {
    assert_send_sync::<Budget>();
    assert_send_sync::<BudgetSnapshot>();
}

fn budget_with(monthly: u64, session: u64) -> Budget {
    let mut caps = HashMap::new();
    caps.insert(
        Provider::ClaudeCode,
        Cap {
            monthly_micro_dollars: monthly,
            session_micro_dollars: session,
        },
    );
    Budget::new(caps)
}

#[tokio::test]
async fn try_charge_under_cap_succeeds_and_returns_healthy_tier() {
    let budget = budget_with(1_000_000, 500_000);
    let tier = budget
        .try_charge(Provider::ClaudeCode, 100_000)
        .await
        .expect("charge under cap succeeds");
    assert_eq!(tier, BudgetTier::Healthy);
}

#[tokio::test]
async fn try_charge_at_50pct_returns_warning() {
    let budget = budget_with(1_000_000, 1_000_000);
    let tier = budget
        .try_charge(Provider::ClaudeCode, 500_000)
        .await
        .expect("charge to 50% succeeds");
    assert_eq!(tier, BudgetTier::Warning);
}

#[tokio::test]
async fn try_charge_at_90pct_returns_critical() {
    let budget = budget_with(1_000_000, 1_000_000);
    let tier = budget
        .try_charge(Provider::ClaudeCode, 900_000)
        .await
        .expect("charge to 90% succeeds");
    assert_eq!(tier, BudgetTier::Critical);
}

#[tokio::test]
async fn try_charge_over_monthly_cap_fails_with_monthlycapexceeded() {
    let budget = budget_with(1_000_000, 10_000_000);
    let err = budget
        .try_charge(Provider::ClaudeCode, 1_000_001)
        .await
        .expect_err("over monthly cap should fail");
    assert_eq!(err, BudgetError::MonthlyCapExceeded(Provider::ClaudeCode));

    // Subsequent attempts should also fail because the provider has latched
    // into the Halted tier.
    let err = budget
        .try_charge(Provider::ClaudeCode, 1)
        .await
        .expect_err("subsequent charge should also fail");
    assert_eq!(err, BudgetError::MonthlyCapExceeded(Provider::ClaudeCode));
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Halted
    );
}

#[tokio::test]
async fn try_charge_over_session_cap_fails_with_sessioncapexceeded() {
    let budget = budget_with(10_000_000, 1_000);
    let err = budget
        .try_charge(Provider::ClaudeCode, 1_001)
        .await
        .expect_err("over session cap should fail");
    assert_eq!(err, BudgetError::SessionCapExceeded(Provider::ClaudeCode));
    // Monthly tier should not be Halted because we never wrote totals.
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Healthy
    );
}

#[tokio::test]
async fn reset_session_zeroes_session_only() {
    let budget = budget_with(10_000_000, 5_000_000);
    budget
        .try_charge(Provider::ClaudeCode, 2_000_000)
        .await
        .expect("charge");
    let snap = budget.snapshot().await;
    assert_eq!(snap.session.get(&Provider::ClaudeCode).copied(), Some(2_000_000));
    assert_eq!(snap.monthly.get(&Provider::ClaudeCode).copied(), Some(2_000_000));

    budget.reset_session().await;
    let snap = budget.snapshot().await;
    assert!(snap.session.get(&Provider::ClaudeCode).copied().unwrap_or(0) == 0);
    assert_eq!(snap.monthly.get(&Provider::ClaudeCode).copied(), Some(2_000_000));
}

#[tokio::test]
async fn reset_monthly_zeroes_monthly_only() {
    let budget = budget_with(10_000_000, 10_000_000);
    budget
        .try_charge(Provider::ClaudeCode, 9_500_000)
        .await
        .expect("charge to Critical");
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Critical
    );

    budget.reset_monthly().await;
    let snap = budget.snapshot().await;
    assert!(snap.monthly.get(&Provider::ClaudeCode).copied().unwrap_or(0) == 0);
    assert_eq!(snap.session.get(&Provider::ClaudeCode).copied(), Some(9_500_000));
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Healthy
    );
}

#[tokio::test]
async fn snapshot_round_trips_via_serde_json() {
    let budget = budget_with(1_000_000, 1_000_000);
    budget
        .try_charge(Provider::ClaudeCode, 100_000)
        .await
        .expect("charge");

    let snap = budget.snapshot().await;
    let json = serde_json::to_string(&snap).expect("serialize");
    let decoded: BudgetSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, snap);
}

#[tokio::test]
async fn snapshot_round_trips_with_custom_provider() {
    // Exercise the Custom variant in the snapshot serde path too.
    let custom_id = CustomProviderId("acme-llm".to_string());
    let provider = Provider::custom(&custom_id);
    let mut caps = HashMap::new();
    caps.insert(
        provider,
        Cap {
            monthly_micro_dollars: 1_000_000,
            session_micro_dollars: 1_000_000,
        },
    );
    let budget = Budget::new(caps);
    budget.try_charge(provider, 250_000).await.expect("charge");

    let snap = budget.snapshot().await;
    let json = serde_json::to_string(&snap).expect("serialize");
    let decoded: BudgetSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, snap);
}

#[tokio::test]
async fn concurrent_charges_are_atomic() {
    // 100 parallel `try_charge(provider, 1)` against a budget with
    // monthly_cap=50 should produce exactly 50 successes and 50 failures.
    let mut caps = HashMap::new();
    caps.insert(
        Provider::ClaudeCode,
        Cap {
            monthly_micro_dollars: 50,
            session_micro_dollars: u64::MAX,
        },
    );
    let budget = Arc::new(Budget::new(caps));

    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let b = Arc::clone(&budget);
        handles.push(tokio::spawn(async move {
            b.try_charge(Provider::ClaudeCode, 1).await
        }));
    }

    let mut ok = 0usize;
    let mut over_monthly = 0usize;
    for h in handles {
        match h.await.expect("join") {
            Ok(_tier) => ok += 1,
            Err(BudgetError::MonthlyCapExceeded(_)) => over_monthly += 1,
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!(ok, 50, "expected exactly 50 successful charges");
    assert_eq!(over_monthly, 50, "expected exactly 50 MonthlyCapExceeded");

    // After the storm the tier must be Halted.
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Halted
    );
}

#[tokio::test]
async fn tier_transitions_are_monotonic() {
    // Once a provider is Halted, no successful charge can return a
    // non-Halted tier. The only way out is a reset.
    let budget = budget_with(100, 10_000);
    budget
        .try_charge(Provider::ClaudeCode, 100)
        .await
        .expect("charge to 100%");
    // 100% of 100 = Halted by the classifier; try_charge will refuse
    // further attempts.
    assert_eq!(
        budget.current_tier(Provider::ClaudeCode).await.unwrap(),
        BudgetTier::Halted
    );

    for _ in 0..10 {
        let result = budget.try_charge(Provider::ClaudeCode, 1).await;
        assert_eq!(
            result,
            Err(BudgetError::MonthlyCapExceeded(Provider::ClaudeCode)),
            "post-Halted charge must continue to fail"
        );
        assert_eq!(
            budget.current_tier(Provider::ClaudeCode).await.unwrap(),
            BudgetTier::Halted,
            "tier must remain Halted across failed charges"
        );
    }
}

#[tokio::test]
async fn unknown_provider_returns_unknown_provider_error() {
    let budget = budget_with(1_000, 1_000);
    let err = budget
        .try_charge(Provider::Codex, 1)
        .await
        .expect_err("unconfigured provider must fail");
    assert_eq!(err, BudgetError::UnknownProvider(Provider::Codex));

    let err = budget
        .current_tier(Provider::Codex)
        .await
        .expect_err("unconfigured provider must fail");
    assert_eq!(err, BudgetError::UnknownProvider(Provider::Codex));
}

#[tokio::test]
async fn evaluate_charge_matches_try_charge() {
    let budget = budget_with(1_000_000, 1_000_000);
    let tier = orchestrator::evaluate_charge(&budget, Provider::ClaudeCode, 250_000)
        .await
        .expect("evaluate_charge succeeds");
    assert_eq!(tier, BudgetTier::Healthy);
    let snap = budget.snapshot().await;
    assert_eq!(snap.monthly.get(&Provider::ClaudeCode).copied(), Some(250_000));
}
