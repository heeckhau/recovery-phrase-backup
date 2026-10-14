// GF(256) arithmetic, the same finite field AES uses: bytes as polynomial
// representations, multiplication reduced modulo the Rijndael irreducible
// polynomial x^8 + x^4 + x^3 + x + 1 (0x11B). See SLIP-39 spec, "Shamir's
// secret-sharing".
//
// mul()/inverse() (the two operations actually used on secret share bytes
// during split/recover) are written to avoid secret-dependent branches and
// secret-indexed memory access: plain array indexing by a secret byte is
// vulnerable in principle to cache-timing analysis. This is a best-effort,
// software-level mitigation (no assembly-verified guarantee, and Rust/LLVM
// could in principle undo some of it under aggressive optimization: there
// is no fully airtight way to guarantee this from safe Rust alone), using
// `subtle` rather than hand-rolled bitmasking, since it's the standard,
// widely-reviewed crate for exactly this. Real-world exploitability was
// already low for this tool's threat model (local, offline, single-user,
// no remote attacker positioned to measure timing): this closes the gap
// properly anyway since the fix is small and self-contained.
//
// Table *construction* (below) only ever processes the public generator
// constant, never secret data, so it's untouched by any of this.
use std::sync::LazyLock;

use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

const POLY: u16 = 0x11B;

/// Multiplies by x (the "xtime" operation), reducing modulo POLY on overflow.
fn xtime(a: u8) -> u8 {
    let shifted = (a as u16) << 1;
    if shifted & 0x100 != 0 {
        (shifted ^ POLY) as u8
    } else {
        shifted as u8
    }
}

/// Direct (non-table) multiplication via shift-and-add ("peasant
/// multiplication"), reducing with `xtime`. Used both to build the log/exp
/// tables below and, independently, as a correctness oracle in tests. Only
/// ever called with the public generator constant, never secret data, so
/// it has no need to be constant-time itself.
fn mul_direct(mut a: u8, mut b: u8) -> u8 {
    let mut result = 0u8;
    for _ in 0..8 {
        if b & 1 == 1 {
            result ^= a;
        }
        b >>= 1;
        a = xtime(a);
    }
    result
}

struct Tables {
    /// exp[i] = GENERATOR^i, for i in 0..255 (the full multiplicative cycle).
    exp: [u8; 255],
    /// log[exp[i]] = i. Index 0 is unused (0 has no logarithm).
    log: [u8; 256],
}

// 3 is a generator of GF(256)*: its multiplicative order is 255 (confirmed
// against mul_direct in tests below). 2 is not (it only has order 51) so
// it can't be used to build a complete table.
const GENERATOR: u8 = 3;

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let mut exp = [0u8; 255];
    let mut log = [0u8; 256];
    let mut x: u8 = 1;
    for (i, slot) in exp.iter_mut().enumerate() {
        *slot = x;
        log[x as usize] = i as u8;
        x = mul_direct(x, GENERATOR);
    }
    Tables { exp, log }
});

/// GF(256) addition (and subtraction: they're the same operation in
/// characteristic 2). A single XOR: already constant-time, no lookup.
pub fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Constant-time table lookup: scans every entry rather than indexing
/// directly by `index`, so which memory is touched doesn't depend on it.
fn ct_lookup(table: &[u8], index: u8) -> u8 {
    let mut result = 0u8;
    for (i, &value) in table.iter().enumerate() {
        let is_match = (i as u8).ct_eq(&index);
        result = u8::conditional_select(&result, &value, is_match);
    }
    result
}

/// Reduces a sum of two log values (range 0..=508) into 0..=254 with one
/// conditional subtraction, rather than `%`, which for a non-power-of-two
/// divisor isn't guaranteed to be constant-time.
fn reduce_log_sum(sum: u16) -> u8 {
    let subtracted = sum.wrapping_sub(255);
    let underflowed = Choice::from((subtracted >> 15) as u8); // top bit set => sum was < 255
    u16::conditional_select(&subtracted, &sum, underflowed) as u8
}

/// `(255 - log_a) mod 255`, branchless. `log_a` is in 0..=254, so
/// `255 - log_a` is always in 1..=255 already: the only reduction ever
/// needed is turning a literal 255 into 0.
fn negate_log(log_a: u8) -> u8 {
    let raw = 255 - log_a;
    let is_255 = raw.ct_eq(&255);
    u8::conditional_select(&raw, &0, is_255)
}

/// GF(256) multiplication, via log/exp tables.
pub fn mul(a: u8, b: u8) -> u8 {
    // Whether an operand is exactly zero is the one thing this leaks in
    // non-constant time: log(0) is undefined, so a log-table approach
    // can't avoid this check without a different algorithm entirely.
    // Everything past it is constant-time in both a and b.
    if a == 0 || b == 0 {
        return 0;
    }
    let t = &*TABLES;
    let log_a = ct_lookup(&t.log, a);
    let log_b = ct_lookup(&t.log, b);
    ct_lookup(&t.exp, reduce_log_sum(log_a as u16 + log_b as u16))
}

/// GF(256) multiplicative inverse. `None` for 0, which has no inverse.
pub fn inverse(a: u8) -> Option<u8> {
    if a == 0 {
        return None;
    }
    let t = &*TABLES;
    let log_a = ct_lookup(&t.log, a);
    Some(ct_lookup(&t.exp, negate_log(log_a)))
}

/// GF(256) division: `a / b = a * b⁻¹`. `None` if `b` is 0.
pub fn div(a: u8, b: u8) -> Option<u8> {
    inverse(b).map(|b_inv| mul(a, b_inv))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_has_full_order_255() {
        // exp[] must contain all 255 nonzero byte values exactly once.
        let mut seen: Vec<u8> = TABLES.exp.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 255);
        assert!(!seen.contains(&0));
    }

    #[test]
    fn table_multiplication_matches_direct_multiplication_exhaustively() {
        for a in 0..=255u16 {
            for b in 0..=255u16 {
                assert_eq!(mul(a as u8, b as u8), mul_direct(a as u8, b as u8), "mismatch for {a}*{b}");
            }
        }
    }

    #[test]
    fn multiplying_by_zero_is_zero() {
        for a in 0..=255u8 {
            assert_eq!(mul(a, 0), 0);
            assert_eq!(mul(0, a), 0);
        }
    }

    #[test]
    fn every_nonzero_element_times_its_inverse_is_one() {
        for a in 1..=255u8 {
            let inv = inverse(a).unwrap();
            assert_eq!(mul(a, inv), 1, "failed for a={a}");
        }
    }

    #[test]
    fn zero_has_no_inverse() {
        assert_eq!(inverse(0), None);
    }

    #[test]
    fn division_undoes_multiplication() {
        for a in 1..=255u8 {
            for b in 1..=255u8 {
                let product = mul(a, b);
                assert_eq!(div(product, b), Some(a));
            }
        }
    }

    #[test]
    fn ct_lookup_matches_direct_indexing_exhaustively() {
        let table: Vec<u8> = (0..=255u8).map(|i| i.wrapping_mul(7).wrapping_add(13)).collect();
        for index in 0..=255u8 {
            assert_eq!(ct_lookup(&table, index), table[index as usize]);
        }
    }
}
