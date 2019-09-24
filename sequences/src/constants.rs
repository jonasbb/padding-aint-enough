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
/// * Phase 2 optimization value: `3`
pub(crate) const SWAP_COST: usize = 3;

pub mod common_sequence_classifications {
    // These patterns were generated for traces using DNSSEC
    pub const R001: &str = "R001 Single Domain. A + DNSKEY";
    pub const R002: &str = "R002 Single Domain with www redirect. A + DNSKEY + A (for www)";
    pub const R003: &str = "R003 Two domains for website. (A + DNSKEY) * 2";
    pub const R004_SIZE1: &str = "R004 Single packet of size 1.";
    pub const R004_SIZE2: &str = "R004 Single packet of size 2.";
    pub const R004_SIZE3: &str = "R004 Single packet of size 3.";
    pub const R004_SIZE4: &str = "R004 Single packet of size 4.";
    pub const R004_SIZE5: &str = "R004 Single packet of size 5.";
    pub const R004_SIZE6: &str = "R004 Single packet of size 6.";
    pub const R004_UNKNOWN: &str = "R004 A single packet of unknown size.";
    pub const R005: &str = "R005 Two domains for website second is CNAME.";
    pub const R006: &str = "R006 www redirect + Akamai";
    pub const R006_3RD_LVL_DOM: &str =
        "R006 www redirect + Akamai on 3rd-LVL domain without DNSSEC";
    pub const R007: &str = "R007 Unreachable Name Server";
    pub const R008: &str =
        "R008 Domain did not load properly and Chrome performed a Google search on the error page.";
    pub const R009: &str = "R009 No network response received.";

    // These patterns are intended for traces without DNSSEC
    pub const R102: &str = "R102 Single Domain with www redirect. A + A (for www)";
    pub const R102A: &str = "R102A Single Domain with www redirect. A + A (for www). Missing gap.";
    pub const R103: &str = "R103 Three Domain requests. Can sometimes be R102 with an erroneous `ssl.gstatic.com` or similar.";
    pub const R103A: &str = "R103A Three Domain requests. Missing first gap.";
    pub const R103B: &str = "R103B Three Domain requests. Missing second gap.";
    pub const R103C: &str = "R103C Three Domain requests. No gaps.";
    pub const R104A: &str = "R104A Four Domain requests. No gaps.";
    pub const R104B: &str = "R104B Four Domain requests. Gap after one.";
    pub const R104C: &str = "R104C Four Domain requests. Gap after two.";
    pub const R104D: &str = "R104D Four Domain requests. Gap after three.";
    pub const R104E: &str = "R104E Four Domain requests.";
    pub const R104F: &str = "R104F Four Domain requests.";
    pub const R104G: &str = "R104G Four Domain requests.";
    pub const R105A: &str = "R105A Five Domain requests. No gaps.";
    pub const R105B: &str = "R105B Five Domain requests.";
    pub const R105C: &str = "R105C Five Domain requests.";
    pub const R105D: &str = "R105D Five Domain requests.";
    pub const R105E: &str = "R105E Five Domain requests.";
    pub const R106A: &str = "R106A Five Domain requests. No gaps.";
}
