//! Per-provider budget accounting with tier transitions and circuit-breaker
//! semantics.
//!
//! The [`Budget`] tracks running spend (in micro-dollars) against per-provider
//! monthly and per-session caps. It is the single source of truth the router
//! consults before dispatching a task to a billable provider, and the source
//! of telemetry the UI reads to display "you've spent X of Y".
//!
//! # Design
//!
//! * State lives behind a single [`tokio::sync::RwLock`] so that
//!   [`Budget::try_charge`] can perform the read-tier-then-write-totals step
//!   atomically. The atomicity test exercises this under contention.
//! * Spend is tracked as `u64` micro-dollars (`$1.00 == 1_000_000`). This
//!   avoids floating-point drift across thousands of small charges.
//! * The Budget itself does not observe wall-clock time; callers are
//!   responsible for invoking [`Budget::reset_session`] /
//!   [`Budget::reset_monthly`] at the appropriate boundaries. Keeping the
//!   clock out of this module makes it trivially deterministic to test.
//! * Once a provider transitions to [`BudgetTier::Halted`] no further charges
//!   succeed until a reset; this is the circuit-breaker behaviour required
//!   by the orchestrator's hard-cap rule.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

/// Stable, human-readable identifier for a non-builtin LLM provider.
///
/// Kept as a separate newtype rather than embedded directly inside
/// [`Provider::Custom`] so that [`Provider`] itself can stay `Copy` (it is
/// used heavily as a `HashMap` key and as a `Copy` function argument). The
/// inner string is treated as the entire identity of the custom provider —
/// two `CustomProviderId`s with the same string compare equal.
///
/// The mapping between [`CustomProviderId`] and the `u32` handle inside
/// [`Provider::Custom`] is owned by the caller. A common pattern is to
/// assign handles in registration order from a counter shared with the
/// [`Budget`] config. See [`Provider::custom_handle`] for a stable hash if
/// the caller does not want to maintain its own registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomProviderId(pub String);

/// Identifies an LLM provider for budget accounting.
///
/// Builtin providers are enumerated explicitly so accounting code does not
/// have to special-case strings. The [`Provider::Custom`] variant carries a
/// `u32` handle that the caller assigns to a [`CustomProviderId`]; see that
/// type's docs for the convention.
///
/// `Provider` is `Copy` so it can be passed by value without ceremony and
/// used as a `HashMap` key without cloning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    /// Anthropic's Claude Code coding agent.
    ClaudeCode,
    /// OpenAI's Codex CLI agent.
    Codex,
    /// Local Ollama runtime.
    Ollama,
    /// Apple Foundation Models (on-device).
    FoundationModels,
    /// A user-registered custom provider, identified by a 32-bit handle the
    /// caller maps to a [`CustomProviderId`].
    Custom(u32),
}

impl Provider {
    /// Derive a stable 32-bit handle from a [`CustomProviderId`] using a
    /// FNV-1a hash of its string contents.
    ///
    /// Provided as a convenience for callers that do not want to maintain
    /// their own `CustomProviderId -> u32` registry. Hash collisions are
    /// possible but vanishingly unlikely for the small number of custom
    /// providers a single deployment will configure.
    pub fn custom_handle(id: &CustomProviderId) -> u32 {
        let mut hash: u32 = 0x811c_9dc5;
        for byte in id.0.as_bytes() {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        hash
    }

    /// Construct a [`Provider::Custom`] from a [`CustomProviderId`] using
    /// [`Self::custom_handle`].
    pub fn custom(id: &CustomProviderId) -> Self {
        Provider::Custom(Self::custom_handle(id))
    }
}

/// Per-provider spending caps.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Cap {
    /// Per-month cap in micro-dollars (i.e. `$1.00 == 1_000_000`).
    pub monthly_micro_dollars: u64,
    /// Per-session cap in micro-dollars.
    pub session_micro_dollars: u64,
}

/// Tier classification derived from current monthly spend.
///
/// The router reads this to decide whether to throttle, downgrade or refuse
/// new work. Thresholds are computed from the *post-charge* monthly total as
/// a percentage of the configured monthly cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BudgetTier {
    /// Under 50% of monthly cap.
    Healthy,
    /// 50%-90%: agents can still spend but Router prefers cheaper agents.
    Warning,
    /// 90%-100%: only critical Roles (Planner/Reviewer) can spend.
    Critical,
    /// `>= 100%`: all spend halts.
    Halted,
}

/// Errors returned by [`Budget::try_charge`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BudgetError {
    /// The charge would push the provider over its monthly cap, or the
    /// provider is already [`BudgetTier::Halted`].
    #[error("Provider {0:?} hit its monthly cap; spend halted")]
    MonthlyCapExceeded(Provider),
    /// The charge would push the provider over its session cap.
    #[error("Provider {0:?} hit its session cap")]
    SessionCapExceeded(Provider),
    /// The provider has no configured [`Cap`] in this [`Budget`].
    #[error("Unknown provider {0:?}")]
    UnknownProvider(Provider),
}

/// Snapshot of all per-provider running totals and tiers.
///
/// Returned by [`Budget::snapshot`] for telemetry and UI display. Implements
/// `Serialize`/`Deserialize` so it can be shipped over the wire as-is.
///
/// Note: the per-provider tables are exposed as `HashMap<Provider, _>` for
/// ergonomic access at call sites. Because `Provider` is an enum (not a
/// string), `serde_json` cannot serialize a `HashMap<Provider, _>` directly;
/// the snapshot therefore implements `Serialize`/`Deserialize` manually,
/// emitting each table as a JSON array of `[provider, value]` pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetSnapshot {
    /// Per-provider monthly spend totals, in micro-dollars.
    pub monthly: HashMap<Provider, u64>,
    /// Per-provider session spend totals, in micro-dollars.
    pub session: HashMap<Provider, u64>,
    /// Per-provider current tier classification.
    pub tier: HashMap<Provider, BudgetTier>,
}

#[derive(Serialize, Deserialize)]
struct BudgetSnapshotWire {
    monthly: Vec<(Provider, u64)>,
    session: Vec<(Provider, u64)>,
    tier: Vec<(Provider, BudgetTier)>,
}

impl Serialize for BudgetSnapshot {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let wire = BudgetSnapshotWire {
            monthly: self.monthly.iter().map(|(k, v)| (*k, *v)).collect(),
            session: self.session.iter().map(|(k, v)| (*k, *v)).collect(),
            tier: self.tier.iter().map(|(k, v)| (*k, *v)).collect(),
        };
        wire.serialize(s)
    }
}

impl<'de> Deserialize<'de> for BudgetSnapshot {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let wire = BudgetSnapshotWire::deserialize(d)?;
        Ok(BudgetSnapshot {
            monthly: wire.monthly.into_iter().collect(),
            session: wire.session.into_iter().collect(),
            tier: wire.tier.into_iter().collect(),
        })
    }
}

#[derive(Debug, Default)]
struct BudgetState {
    monthly: HashMap<Provider, u64>,
    session: HashMap<Provider, u64>,
    tier: HashMap<Provider, BudgetTier>,
    /// When the current month window started — used for monthly reset.
    month_anchor: Option<Instant>,
}

/// Per-provider budget tracker.
///
/// A `Budget` is cheap to clone via [`Arc`] but is typically held as a
/// long-lived single instance behind the orchestrator. All accessors are
/// `async` and serialize through an internal lock — see the module-level
/// docs for the atomicity guarantee.
///
/// [`Arc`]: std::sync::Arc
pub struct Budget {
    caps: HashMap<Provider, Cap>,
    state: Arc<RwLock<BudgetState>>,
}

impl Budget {
    /// Construct a new `Budget` with the given per-provider caps.
    ///
    /// Providers absent from `caps` will be rejected by [`Self::try_charge`]
    /// with [`BudgetError::UnknownProvider`].
    pub fn new(caps: HashMap<Provider, Cap>) -> Self {
        let mut state = BudgetState::default();
        for provider in caps.keys() {
            state.tier.insert(*provider, BudgetTier::Healthy);
        }
        state.month_anchor = Some(Instant::now());
        Self {
            caps,
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Atomically check the tier, refuse if [`BudgetTier::Halted`] or if the
    /// charge would push over either cap, otherwise add to running totals
    /// and return the new tier.
    ///
    /// The tier returned is computed from the **after-charge** monthly total.
    pub async fn try_charge(
        &self,
        provider: Provider,
        micro_dollars: u64,
    ) -> Result<BudgetTier, BudgetError> {
        let cap = self
            .caps
            .get(&provider)
            .copied()
            .ok_or(BudgetError::UnknownProvider(provider))?;

        let mut state = self.state.write().await;

        // Circuit-breaker: once Halted, stay Halted until a reset clears
        // the running totals. The tier map records this independently of
        // the integer totals so that monotonicity holds across resets too:
        // see `tier_transitions_are_monotonic`.
        if matches!(state.tier.get(&provider), Some(BudgetTier::Halted)) {
            return Err(BudgetError::MonthlyCapExceeded(provider));
        }

        let current_monthly = state.monthly.get(&provider).copied().unwrap_or(0);
        let current_session = state.session.get(&provider).copied().unwrap_or(0);

        let new_monthly = current_monthly
            .checked_add(micro_dollars)
            .ok_or(BudgetError::MonthlyCapExceeded(provider))?;
        let new_session = current_session
            .checked_add(micro_dollars)
            .ok_or(BudgetError::SessionCapExceeded(provider))?;

        if new_monthly > cap.monthly_micro_dollars {
            // Latch the Halted tier so subsequent calls fail fast even if
            // the caller asks for tiny charges that would fit.
            state.tier.insert(provider, BudgetTier::Halted);
            return Err(BudgetError::MonthlyCapExceeded(provider));
        }
        if new_session > cap.session_micro_dollars {
            return Err(BudgetError::SessionCapExceeded(provider));
        }

        let new_tier = classify_tier(new_monthly, cap.monthly_micro_dollars);
        state.monthly.insert(provider, new_monthly);
        state.session.insert(provider, new_session);
        state.tier.insert(provider, new_tier);
        Ok(new_tier)
    }

    /// Read the current tier for `provider` without modifying state.
    pub async fn current_tier(&self, provider: Provider) -> Result<BudgetTier, BudgetError> {
        if !self.caps.contains_key(&provider) {
            return Err(BudgetError::UnknownProvider(provider));
        }
        let state = self.state.read().await;
        Ok(state
            .tier
            .get(&provider)
            .copied()
            .unwrap_or(BudgetTier::Healthy))
    }

    /// Zero out the session column for all providers.
    ///
    /// Called at task-boundary or session-boundary by the orchestrator. Does
    /// not affect monthly totals or the tier classification (which is
    /// derived from monthly spend).
    pub async fn reset_session(&self) {
        let mut state = self.state.write().await;
        state.session.clear();
    }

    /// Zero out the monthly column for all providers and reset the month
    /// anchor.
    ///
    /// The caller is responsible for invoking this on month rollover; the
    /// `Budget` itself does not observe wall-clock time. Resetting monthly
    /// also clears any latched [`BudgetTier::Halted`] state, since the
    /// tier-from-spend invariant would otherwise be violated.
    pub async fn reset_monthly(&self) {
        let mut state = self.state.write().await;
        state.monthly.clear();
        for tier in state.tier.values_mut() {
            *tier = BudgetTier::Healthy;
        }
        state.month_anchor = Some(Instant::now());
    }

    /// Return a serializable snapshot of all per-provider state.
    pub async fn snapshot(&self) -> BudgetSnapshot {
        let state = self.state.read().await;
        BudgetSnapshot {
            monthly: state.monthly.clone(),
            session: state.session.clone(),
            tier: state.tier.clone(),
        }
    }
}

/// Convenience wrapper around [`Budget::try_charge`] for use at router call
/// sites. Semantics are identical; this exists purely for readability where
/// the caller wants to spell out "we are about to evaluate a billable charge".
pub async fn evaluate_charge(
    budget: &Budget,
    provider: Provider,
    estimated_micros: u64,
) -> Result<BudgetTier, BudgetError> {
    budget.try_charge(provider, estimated_micros).await
}

/// Tier-from-spend classification.
///
/// Thresholds are computed against the configured monthly cap:
/// * `< 50%`  → [`BudgetTier::Healthy`]
/// * `50–90%` → [`BudgetTier::Warning`]
/// * `90–100%` → [`BudgetTier::Critical`]
/// * `>= 100%` → [`BudgetTier::Halted`] (rejected by [`Budget::try_charge`])
///
/// Comparisons use cross-multiplication on `u128` to avoid floating-point
/// rounding artefacts — important because the Critical/Halted boundary is a
/// hard cap.
fn classify_tier(monthly_spend: u64, monthly_cap: u64) -> BudgetTier {
    if monthly_cap == 0 {
        // Degenerate cap: any successful charge means we are at or above 100%.
        return if monthly_spend == 0 {
            BudgetTier::Healthy
        } else {
            BudgetTier::Halted
        };
    }
    let spend = monthly_spend as u128;
    let cap = monthly_cap as u128;
    if spend * 100 >= cap * 100 {
        // spend >= cap
        if spend >= cap {
            return BudgetTier::Halted;
        }
    }
    if spend * 10 >= cap * 9 {
        return BudgetTier::Critical;
    }
    if spend * 2 >= cap {
        return BudgetTier::Warning;
    }
    BudgetTier::Healthy
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn classify_tier_thresholds() {
        // 0% -> Healthy
        assert_eq!(classify_tier(0, 100), BudgetTier::Healthy);
        // 49% -> Healthy
        assert_eq!(classify_tier(49, 100), BudgetTier::Healthy);
        // 50% -> Warning
        assert_eq!(classify_tier(50, 100), BudgetTier::Warning);
        // 89% -> Warning
        assert_eq!(classify_tier(89, 100), BudgetTier::Warning);
        // 90% -> Critical
        assert_eq!(classify_tier(90, 100), BudgetTier::Critical);
        // 99% -> Critical
        assert_eq!(classify_tier(99, 100), BudgetTier::Critical);
        // 100% -> Halted
        assert_eq!(classify_tier(100, 100), BudgetTier::Halted);
    }
}
