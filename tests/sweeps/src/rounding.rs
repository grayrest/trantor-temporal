//! ECMA-402's unsigned rounding modes and Temporal's RoundNumberToIncrement,
//! in integer arithmetic, shared by the rounding oracles.
use temporal_rs::options::RoundingMode as M;

#[derive(Clone, Copy, PartialEq)]
pub enum Unsigned { Zero, Infinity, HalfZero, HalfInfinity, HalfEven }

/// ECMA-402 GetUnsignedRoundingMode.
pub fn unsigned(mode: M, negative: bool) -> Unsigned {
    match (mode, negative) {
        (M::Ceil, false) | (M::Floor, true) | (M::Expand, _) => Unsigned::Infinity,
        (M::Ceil, true) | (M::Floor, false) | (M::Trunc, _) => Unsigned::Zero,
        (M::HalfCeil, false) | (M::HalfFloor, true) | (M::HalfExpand, _) => Unsigned::HalfInfinity,
        (M::HalfCeil, true) | (M::HalfFloor, false) | (M::HalfTrunc, _) => Unsigned::HalfZero,
        (M::HalfEven, _) => Unsigned::HalfEven,
    }
}

/// ECMA-402 ApplyUnsignedRoundingMode for x = num / den with r1 <= x <= r2,
/// r2 - r1 the increment.
pub fn apply(num: i128, den: i128, r1: i128, r2: i128, mode: Unsigned) -> i128 {
    if num == r1 * den { return r1; }
    match mode {
        Unsigned::Zero => return r1,
        Unsigned::Infinity => return r2,
        _ => {}
    }
    let (d1, d2) = (num - r1 * den, r2 * den - num);
    if d1 < d2 { return r1; }
    if d2 < d1 { return r2; }
    match mode {
        Unsigned::HalfZero => r1,
        Unsigned::HalfInfinity => r2,
        _ => if (r1 / (r2 - r1)).rem_euclid(2) == 0 { r1 } else { r2 },
    }
}

/// RoundNumberToIncrement, signed.
pub fn round_to_increment(x: i128, increment: i128, mode: M) -> i128 {
    let negative = x < 0;
    let q = x.abs();
    let r1 = q / increment;
    let rounded = apply(q, increment, r1, r1 + 1, unsigned(mode, negative));
    (if negative { -rounded } else { rounded }) * increment
}
