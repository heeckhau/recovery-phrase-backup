// GF(256) arithmetic — the same finite field AES uses: bytes as polynomial
// representations, multiplication reduced modulo the Rijndael irreducible
// polynomial x^8 + x^4 + x^3 + x + 1 (0x11B). See SLIP-39 spec, "Shamir's
// secret-sharing".
use std::sync::LazyLock;

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
/// tables below and, independently, as a correctness oracle in tests.
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
// against mul_direct in tests below). 2 is not — it only has order 51 — so
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

/// GF(256) addition (and subtraction — they're the same operation in
/// characteristic 2).
pub fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// GF(256) multiplication, via log/exp tables.
pub fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let t = &*TABLES;
    let sum = t.log[a as usize] as u16 + t.log[b as usize] as u16;
    t.exp[(sum % 255) as usize]
}

/// GF(256) multiplicative inverse. `None` for 0, which has no inverse.
pub fn inverse(a: u8) -> Option<u8> {
    if a == 0 {
        return None;
    }
    let t = &*TABLES;
    let log_a = t.log[a as usize] as u16;
    Some(t.exp[((255 - log_a) % 255) as usize])
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
}
