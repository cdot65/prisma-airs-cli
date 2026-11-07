//! Numeric bounds shared by every JSON count.

/// JavaScript's `Number.MAX_SAFE_INTEGER` (2^53 − 1), the bound of every JSON count.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
