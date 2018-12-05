//! Specify some constants used in the Damerau-Levenshtein comparison of Sequences

/// The cost of inserting any Size(_)
///
/// * Pre-optimization default: `20`
/// * Phase 1 optimization value: `28`
pub(crate) const SIZE_INSERT_COST: usize = 28;

/// A multiplier to the Gap value while inserting
///
/// Pre-optimization default: `5`
pub(crate) const GAP_INSERT_COST_MULTIPLIER: usize = 5;

/// Specify how much a substitute from Size->Size should cost compared to insert+delete costs
///
/// Pre-optimization default: `3`
pub(crate) const SIZE_SUBSTITUTE_COST_DIVIDER: usize = 3;

/// Specify a multiplier to the difference in Gap values for a Gap->Gap substitution
///
/// Pre-optimization default: `2`
pub(crate) const GAP_SUBSTITUTE_COST_MULTIPLIER: usize = 2;

/// The cost of swapping two non-equal elements
///
/// Pre-optimization default: `20`
pub(crate) const SWAP_COST: usize = 20;
