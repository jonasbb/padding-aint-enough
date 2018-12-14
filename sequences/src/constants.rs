//! Specify some constants used in the Damerau-Levenshtein comparison of Sequences

/// The cost of inserting any Size(_)
///
/// * Pre-optimization default: `20`
/// * Phase 1 optimization value: `28`
/// * Phase 2 optimization value: `12`
pub(crate) const SIZE_INSERT_COST: usize = 12;

/// A multiplier to the Gap value while inserting
///
/// * Pre-optimization default: `5`
/// * Phase 1 optimization value: `1`
/// * Phase 2 optimization value: `1`
pub(crate) const GAP_INSERT_COST_MULTIPLIER: usize = 1;

/// Specify how much a substitute from Size->Size should cost compared to insert+delete costs
///
/// * Pre-optimization default: `3`
/// * Phase 1 optimization value: `4`
/// * Phase 2 optimization value: `4`
pub(crate) const SIZE_SUBSTITUTE_COST_DIVIDER: usize = 4;

/// Specify a multiplier to the difference in Gap values for a Gap->Gap substitution
///
/// * Pre-optimization default: `2`
/// * Phase 1 optimization value: `3`
/// * Phase 2 optimization value: `3`
pub(crate) const GAP_SUBSTITUTE_COST_MULTIPLIER: usize = 3;

/// The cost of swapping two non-equal elements
///
/// * Pre-optimization default: `20`
/// * Phase 1 optimization value: `3` / `4`
pub(crate) const SWAP_COST: usize = 3;
