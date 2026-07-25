//! Per-run cost, so the governed path can answer "what did this cost" from
//! the same record that answers who, when, which model and how many tokens.
//!
//! Cost is derived, not reported by the backend: the price book maps a policy
//! alias to a rate, and the run's own usage does the rest. An alias the book
//! does not name is recorded as `unpriced` with no amount, never as zero: a
//! fabricated zero is indistinguishable from a genuinely free call.
//!
//! Rates are integer micros (millionths of a currency unit) per 1M tokens,
//! which keeps the arithmetic exact and the stored amount an integer.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::routing::BackendUsage;

/// Metered: the amount is what the provider bills for this run.
pub const COST_BASIS_METERED: &str = "metered";
/// Subscription: the seat is already paid, so the run's marginal cost is
/// zero. Recorded explicitly, because "zero because subscription" and "zero
/// because unknown" are different facts.
pub const COST_BASIS_SUBSCRIPTION: &str = "subscription";
/// No entry in the price book for this alias.
pub const COST_BASIS_UNPRICED: &str = "unpriced";

/// What one model alias costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPrice {
    /// `metered` or `subscription`.
    pub basis: String,
    #[serde(default)]
    pub prompt_micros_per_1m: u64,
    #[serde(default)]
    pub completion_micros_per_1m: u64,
    pub currency: String,
}

/// Cost attributed to a single run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunCost {
    /// `None` when the alias is unpriced or the run reported no usage: an
    /// absent amount is a fact, a zero would be a claim.
    pub cost_micros: Option<u64>,
    pub cost_basis: String,
    pub currency: Option<String>,
}

impl RunCost {
    fn unpriced() -> Self {
        Self {
            cost_micros: None,
            cost_basis: COST_BASIS_UNPRICED.to_owned(),
            currency: None,
        }
    }
}

/// Model alias to price, loaded once at boot.
#[derive(Debug, Clone, Default)]
pub struct PriceBook {
    models: HashMap<String, ModelPrice>,
}

impl PriceBook {
    pub fn new(models: HashMap<String, ModelPrice>) -> Result<Self, String> {
        for (alias, price) in &models {
            match price.basis.as_str() {
                COST_BASIS_METERED => {
                    if price.prompt_micros_per_1m == 0 && price.completion_micros_per_1m == 0 {
                        return Err(format!(
                            "'{alias}' is metered but carries no rate; give it a rate or \
                             declare it as '{COST_BASIS_SUBSCRIPTION}'"
                        ));
                    }
                }
                COST_BASIS_SUBSCRIPTION => {
                    if price.prompt_micros_per_1m != 0 || price.completion_micros_per_1m != 0 {
                        return Err(format!(
                            "'{alias}' is a subscription but carries per-token rates; \
                             a subscription run has no marginal cost"
                        ));
                    }
                }
                other => {
                    return Err(format!(
                        "'{alias}' has unknown basis '{other}'; expected \
                         '{COST_BASIS_METERED}' or '{COST_BASIS_SUBSCRIPTION}'"
                    ))
                }
            }
            if price.currency.trim().is_empty() {
                return Err(format!("'{alias}' has no currency"));
            }
        }
        Ok(Self { models })
    }

    /// Parse the JSON object behind `THALAMUS_MODEL_PRICES`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let models: HashMap<String, ModelPrice> =
            serde_json::from_str(raw).map_err(|e| e.to_string())?;
        Self::new(models)
    }

    /// Price book from `THALAMUS_MODEL_PRICES`. Unset is an empty book: every
    /// run is recorded as `unpriced`, which is honest and visible in the
    /// ledger rather than silently absent.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("THALAMUS_MODEL_PRICES") {
            Ok(raw) if !raw.trim().is_empty() => Self::parse(&raw),
            _ => Ok(Self::default()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Cost of one run at this alias, given what it actually consumed.
    pub fn cost_of(&self, model_alias: &str, usage: &BackendUsage) -> RunCost {
        let Some(price) = self.models.get(model_alias) else {
            return RunCost::unpriced();
        };
        if price.basis == COST_BASIS_SUBSCRIPTION {
            return RunCost {
                cost_micros: Some(0),
                cost_basis: COST_BASIS_SUBSCRIPTION.to_owned(),
                currency: Some(price.currency.clone()),
            };
        }

        // Metered. Without token counts there is nothing to bill against, so
        // the amount stays absent while the basis still records the intent.
        let (Some(prompt), Some(completion)) = (
            usage.prompt_tokens.or_else(|| {
                // Some backends report only a total; attribute it to prompt
                // rather than dropping the run from the ledger.
                usage
                    .total_tokens
                    .filter(|_| usage.completion_tokens.is_none())
            }),
            usage.completion_tokens.or(Some(0)),
        ) else {
            return RunCost {
                cost_micros: None,
                cost_basis: COST_BASIS_METERED.to_owned(),
                currency: Some(price.currency.clone()),
            };
        };

        let micros = u128::from(prompt) * u128::from(price.prompt_micros_per_1m) / 1_000_000
            + u128::from(completion) * u128::from(price.completion_micros_per_1m) / 1_000_000;
        RunCost {
            cost_micros: Some(u64::try_from(micros).unwrap_or(u64::MAX)),
            cost_basis: COST_BASIS_METERED.to_owned(),
            currency: Some(price.currency.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(prompt: Option<u32>, completion: Option<u32>, total: Option<u32>) -> BackendUsage {
        BackendUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
        }
    }

    fn metered_book() -> PriceBook {
        PriceBook::parse(
            r#"{"coding.standard":{"basis":"metered","prompt_micros_per_1m":600000,
                "completion_micros_per_1m":2200000,"currency":"USD"},
                "glm-test":{"basis":"subscription","currency":"USD"}}"#,
        )
        .unwrap()
    }

    #[test]
    fn metered_cost_is_exact_integer_arithmetic() {
        let cost = metered_book().cost_of(
            "coding.standard",
            &usage(Some(12_000), Some(3_000), Some(15_000)),
        );
        // 12000 * 600000 / 1e6 = 7200 ; 3000 * 2200000 / 1e6 = 6600
        assert_eq!(cost.cost_micros, Some(13_800));
        assert_eq!(cost.cost_basis, COST_BASIS_METERED);
        assert_eq!(cost.currency.as_deref(), Some("USD"));
    }

    #[test]
    fn subscription_records_zero_marginal_cost_explicitly() {
        let cost = metered_book().cost_of("glm-test", &usage(Some(10), Some(20), Some(30)));
        assert_eq!(cost.cost_micros, Some(0));
        assert_eq!(cost.cost_basis, COST_BASIS_SUBSCRIPTION);
    }

    #[test]
    fn unknown_alias_is_unpriced_never_zero() {
        let cost = metered_book().cost_of("something-else", &usage(Some(10), Some(20), Some(30)));
        assert_eq!(cost.cost_micros, None);
        assert_eq!(cost.cost_basis, COST_BASIS_UNPRICED);
        assert_eq!(cost.currency, None);
    }

    #[test]
    fn metered_without_usage_keeps_the_basis_and_drops_the_amount() {
        let cost = metered_book().cost_of("coding.standard", &usage(None, None, None));
        assert_eq!(cost.cost_micros, None);
        assert_eq!(cost.cost_basis, COST_BASIS_METERED);
    }

    #[test]
    fn total_only_usage_is_still_billed() {
        let cost = metered_book().cost_of("coding.standard", &usage(None, None, Some(1_000)));
        assert_eq!(cost.cost_micros, Some(600));
    }

    #[test]
    fn malformed_books_are_refused_at_parse_time() {
        // Metered without a rate.
        assert!(PriceBook::parse(r#"{"a":{"basis":"metered","currency":"USD"}}"#).is_err());
        // Subscription with a rate.
        assert!(PriceBook::parse(
            r#"{"a":{"basis":"subscription","prompt_micros_per_1m":5,"currency":"USD"}}"#
        )
        .is_err());
        // Unknown basis.
        assert!(PriceBook::parse(r#"{"a":{"basis":"free","currency":"USD"}}"#).is_err());
        // No currency.
        assert!(PriceBook::parse(
            r#"{"a":{"basis":"metered","prompt_micros_per_1m":5,"currency":""}}"#
        )
        .is_err());
        // Unknown field: a typo must not become a silently ignored rate.
        assert!(PriceBook::parse(
            r#"{"a":{"basis":"metered","prompt_micros":5,"currency":"USD"}}"#
        )
        .is_err());
    }

    #[test]
    fn empty_book_prices_nothing() {
        let book = PriceBook::default();
        assert!(book.is_empty());
        assert_eq!(
            book.cost_of("coding.standard", &usage(Some(1), Some(1), Some(2)))
                .cost_basis,
            COST_BASIS_UNPRICED
        );
    }
}
