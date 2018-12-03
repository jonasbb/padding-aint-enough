//! Specify some constants used in the Damerau-Levenshtein comparison of Sequences

/// The cost of inserting any Size(_)
pub(crate) const SIZE_INSERT_COST: usize = 20;

/// A multiplier to the Gap value while inserting
pub(crate) const GAP_INSERT_COST_MULTIPLIER: usize = 5;

/// Specify how much a substitute from Size->Size should cost compared to insert+delete costs
pub(crate) const SIZE_SUBSTITUTE_COST_DIVIDER: usize = 3;

/// Specify a multiplier to the difference in Gap values for a Gap->Gap substitution
pub(crate) const GAP_SUBSTITUTE_COST_MULTIPLIER: usize = 2;

/// The cost of swapping two non-equal elements
pub(crate) const SWAP_COST: usize = 20;
