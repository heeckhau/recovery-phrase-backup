// GF(256) Shamir secret sharing: SplitSecret/RecoverSecret from the SLIP-39
// spec. Used identically at both the group level and the member level of
// the two-level scheme (see lib.rs).
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::error::Error;
use crate::gf256;
use crate::SecretBytes;

type HmacSha256 = Hmac<Sha256>;

/// A share point: its x-coordinate (share index) and secret y value-vector.
pub type Point = (u8, SecretBytes);

/// First 4 bytes of HMAC-SHA256(key, msg): the "digest" used as the fixed
/// point at x=254, verified on recovery to detect wrong/insufficient/
/// corrupted shares.
fn digest4(key: &[u8], msg: &[u8]) -> [u8; 4] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(msg);
    let out = mac.finalize().into_bytes();
    let mut d = [0u8; 4];
    d.copy_from_slice(&out[0..4]);
    d
}

/// Lagrange-interpolates the value-vector at `x` given a set of
/// (x_i, y_i) points, meaningful when the x_i are pairwise distinct.
///
/// The `xj == xi` skip below compares by *value*, so a duplicated
/// x-coordinate is silently absorbed rather than ever reaching the
/// division below with a zero denominator: this can't actually panic on
/// duplicate x_i, even degenerate all-duplicate input, verified directly
/// in this module's tests rather than just reasoned about. It still
/// returns `Result` rather than leaning on that non-obvious property:
/// x-coordinates aren't secret (a share's index is public, visible in the
/// mnemonic itself), so there's no downside to handling this defensively
/// via a typed error instead. (Silently-wrong-but-not-crashing duplicate
/// input still gets caught: `recover_secret`'s HMAC digest check rejects
/// the resulting mismatched secret.)
fn interpolate(x: u8, points: &[Point]) -> Result<SecretBytes, Error> {
    let n = points[0].1.len();
    let mut result = Zeroizing::new(vec![0u8; n]);
    for (xi, yi) in points {
        let mut basis = 1u8;
        for (xj, _) in points {
            if xj == xi {
                continue;
            }
            let num = gf256::add(x, *xj);
            let den = gf256::add(*xi, *xj);
            let ratio = gf256::div(num, den).ok_or(Error::ShareSetInconsistent)?;
            basis = gf256::mul(basis, ratio);
        }
        for k in 0..n {
            result[k] = gf256::add(result[k], gf256::mul(basis, yi[k]));
        }
    }
    Ok(result)
}

/// SplitSecret(T, N, S): splits `secret` into `share_count` shares, any
/// `threshold` of which reconstruct it. If `threshold` is 1, every share
/// simply equals the secret.
pub fn split_secret(threshold: u8, share_count: u8, secret: &[u8]) -> Result<Vec<Point>, Error> {
    if share_count == 0 || share_count > 16 {
        return Err(Error::InvalidShareCount(share_count));
    }
    if threshold == 0 || threshold > share_count {
        return Err(Error::InvalidThreshold { threshold, share_count });
    }
    let bits = secret.len() * 8;
    if bits < 128 || !bits.is_multiple_of(16) {
        return Err(Error::InvalidSecretLength(bits));
    }

    if threshold == 1 {
        return Ok((0..share_count).map(|x| (x, Zeroizing::new(secret.to_vec()))).collect());
    }

    let n = secret.len();
    let mut r = Zeroizing::new(vec![0u8; n - 4]);
    getrandom::fill(&mut r)?;
    let mut d = Zeroizing::new(digest4(&r, secret).to_vec());
    d.extend_from_slice(&r);

    let mut shares: Vec<Point> = Vec::with_capacity(share_count as usize);
    for x in 0..(threshold - 2) {
        let mut y = Zeroizing::new(vec![0u8; n]);
        getrandom::fill(&mut y)?;
        shares.push((x, y));
    }

    let mut interpolation_set = shares.clone();
    interpolation_set.push((254, d));
    interpolation_set.push((255, Zeroizing::new(secret.to_vec())));

    for x in (threshold - 2)..share_count {
        shares.push((x, interpolate(x, &interpolation_set)?));
    }

    Ok(shares)
}

/// RecoverSecret(T, shares): reconstructs the secret from a set of
/// share-index/share-value pairs, verifying the embedded digest. Returns
/// `Error::ShareSetInconsistent` if the shares don't reconstruct a
/// consistent secret: which covers wrong shares, too few shares, and
/// corrupted share data alike, since all of those break the digest check
/// the same way.
pub fn recover_secret(threshold: u8, shares: &[Point]) -> Result<SecretBytes, Error> {
    let Some(first) = shares.first() else {
        return Err(Error::ShareSetInconsistent);
    };

    if threshold == 1 {
        return Ok(first.1.clone());
    }

    let secret = interpolate(255, shares)?;
    let d = interpolate(254, shares)?;

    let r = &d[4..];
    let expected_prefix = digest4(r, &secret);

    // Constant-time comparison: this is a MAC-style integrity check, so
    // the usual textbook mistake is comparing it with a short-circuiting
    // `==`. (Lengths are both fixed at 4 bytes, not secret either way.)
    if bool::from(d[0..4].ct_eq(&expected_prefix)) {
        Ok(secret)
    } else {
        Err(Error::ShareSetInconsistent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret_32_bytes() -> Vec<u8> {
        (0u8..32).collect()
    }

    #[test]
    fn threshold_one_gives_every_share_the_secret_directly() {
        let secret = secret_32_bytes();
        let shares = split_secret(1, 4, &secret).unwrap();
        assert_eq!(shares.len(), 4);
        for (_, y) in &shares {
            assert_eq!(**y, secret);
        }
        assert_eq!(*recover_secret(1, &shares[..1]).unwrap(), secret);
    }

    #[test]
    fn split_then_recover_round_trips_for_various_thresholds() {
        let secret = secret_32_bytes();
        for &(t, n) in &[(2u8, 2u8), (2, 3), (3, 5), (5, 5), (3, 16)] {
            let shares = split_secret(t, n, &secret).unwrap();
            assert_eq!(shares.len(), n as usize);

            // Any t-sized subset must recover the same secret.
            for start in 0..(n - t + 1) {
                let subset = &shares[start as usize..(start + t) as usize];
                assert_eq!(*recover_secret(t, subset).unwrap(), secret, "t={t} n={n} start={start}");
            }
        }
    }

    #[test]
    fn too_few_shares_fails_the_digest_check() {
        let secret = secret_32_bytes();
        let shares = split_secret(3, 5, &secret).unwrap();
        assert_eq!(recover_secret(3, &shares[..2]), Err(Error::ShareSetInconsistent));
    }

    #[test]
    fn corrupted_share_value_fails_the_digest_check() {
        let secret = secret_32_bytes();
        let mut shares = split_secret(3, 5, &secret).unwrap();
        shares[0].1[0] ^= 0xFF;
        assert_eq!(recover_secret(3, &shares[..3]), Err(Error::ShareSetInconsistent));
    }

    #[test]
    fn rejects_invalid_threshold_and_share_count() {
        let secret = secret_32_bytes();
        assert_eq!(split_secret(0, 5, &secret), Err(Error::InvalidThreshold { threshold: 0, share_count: 5 }));
        assert_eq!(split_secret(6, 5, &secret), Err(Error::InvalidThreshold { threshold: 6, share_count: 5 }));
        assert_eq!(split_secret(1, 17, &secret), Err(Error::InvalidShareCount(17)));
        assert_eq!(split_secret(1, 0, &secret), Err(Error::InvalidShareCount(0)));
    }

    #[test]
    fn duplicate_x_coordinates_do_not_panic() {
        // interpolate()'s value-based `xj == xi` skip already absorbs
        // duplicate x-coordinates rather than dividing by zero: verified
        // directly here (not just reasoned about) since it's exactly the
        // kind of subtle invariant worth checking rather than assuming.
        // The duplicated share still can't be used to recover anything:
        // it silently drops out of the Lagrange basis computation, which
        // produces a mismatched secret/digest pair that the HMAC check
        // below rejects: not a crash, but not a successful recovery either.
        let secret = secret_32_bytes();
        let mut shares = split_secret(3, 5, &secret).unwrap();
        shares[1].0 = shares[0].0; // duplicate the first share's x-coordinate
        assert_eq!(recover_secret(3, &shares[..3]), Err(Error::ShareSetInconsistent));
    }

    #[test]
    fn rejects_invalid_secret_length() {
        assert_eq!(split_secret(1, 2, &[0u8; 15]), Err(Error::InvalidSecretLength(120)));
        assert_eq!(split_secret(1, 2, &[0u8; 17]), Err(Error::InvalidSecretLength(136)));
    }
}

// Kani proofs (`cargo kani -p rpb-slip39 -Z stubbing`), for every secret
// and every value the RNG could return. Replaced in the round trips, to
// keep them within reach of the solver:
// - `gf256::mul` and `gf256::inverse` by table-free versions, which
//   `gf256::tests::direct_matches_tables_exhaustively` shows are the same
//   functions;
// - `digest4` (HMAC-SHA256) by a cheap function: a round trip only needs
//   the digest to be the same for the same input.
#[cfg(kani)]
mod proofs {
    use super::*;

    fn stub_digest4(key: &[u8], msg: &[u8]) -> [u8; 4] {
        let mut d = [0x5au8; 4];
        for (i, &b) in key.iter().chain(msg.iter()).enumerate() {
            d[i % 4] = d[i % 4].rotate_left(1) ^ b;
        }
        d
    }

    /// `p(x)` for the polynomial with coefficients `coeffs` (lowest first).
    fn eval(coeffs: &[u8], x: u8) -> u8 {
        coeffs.iter().rev().fold(0, |acc, &c| gf256::add(gf256::direct::mul(acc, x), c))
    }

    /// The fact Shamir sharing rests on: through `K` points with distinct
    /// x-coordinates, `interpolate` evaluates the one polynomial of degree
    /// below `K` that passes through them, at any `x`. For every such
    /// polynomial and every choice of x-coordinates. One byte per value is
    /// enough: `interpolate` handles each byte position on its own.
    fn check_interpolation<const K: usize>() {
        let coeffs: [u8; K] = kani::any();
        let xs: [u8; K] = kani::any();
        for i in 0..K {
            for j in 0..i {
                kani::assume(xs[i] != xs[j]);
            }
        }
        let points: Vec<Point> = xs.iter().map(|&xi| (xi, Zeroizing::new(vec![eval(&coeffs, xi)]))).collect();
        let x: u8 = kani::any();
        assert_eq!(interpolate(x, &points).unwrap()[..], [eval(&coeffs, x)]);
    }

    #[kani::proof]
    #[kani::stub(zeroize::optimization_barrier, crate::kani_stubs::optimization_barrier)]
    #[kani::stub(zeroize::volatile_set, crate::kani_stubs::volatile_set)]
    #[kani::stub(zeroize::volatile_write, crate::kani_stubs::volatile_write)]
    #[kani::stub(gf256::mul, gf256::direct::mul)]
    #[kani::stub(gf256::inverse, gf256::direct::inverse_by_constraint)]
    #[kani::unwind(9)]
    fn interpolation_through_2_points() {
        check_interpolation::<2>();
    }

    #[kani::proof]
    #[kani::stub(zeroize::optimization_barrier, crate::kani_stubs::optimization_barrier)]
    #[kani::stub(zeroize::volatile_set, crate::kani_stubs::volatile_set)]
    #[kani::stub(zeroize::volatile_write, crate::kani_stubs::volatile_write)]
    #[kani::stub(gf256::mul, gf256::direct::mul)]
    #[kani::stub(gf256::inverse, gf256::direct::inverse_by_constraint)]
    #[kani::unwind(9)]
    fn interpolation_through_3_points() {
        check_interpolation::<3>();
    }

    /// Splits an arbitrary 16-byte secret `threshold`-of-`share_count`, then
    /// recovers it from every `threshold`-sized set of shares.
    fn check_round_trip(threshold: u8, share_count: u8) {
        let secret: [u8; 16] = kani::any();
        let shares = split_secret(threshold, share_count, &secret).unwrap();
        assert_eq!(shares.len(), share_count as usize);
        for (i, (x, _)) in shares.iter().enumerate() {
            assert_eq!(*x as usize, i);
        }
        for subset in 0u32..(1 << share_count) {
            // Not `count_ones()`: Kani 0.68 crashes on its intrinsic.
            if (0..share_count).filter(|i| subset & (1 << i) != 0).count() != threshold as usize {
                continue;
            }
            let chosen: Vec<Point> =
                shares.iter().enumerate().filter(|(i, _)| subset & (1 << i) != 0).map(|(_, p)| p.clone()).collect();
            assert_eq!(recover_secret(threshold, &chosen).unwrap()[..], secret[..]);
        }
    }

    #[kani::proof]
    #[kani::stub(getrandom::fill, crate::kani_stubs::fill)]
    #[kani::stub(zeroize::optimization_barrier, crate::kani_stubs::optimization_barrier)]
    #[kani::stub(gf256::mul, gf256::direct::mul)]
    #[kani::stub(gf256::inverse, gf256::direct::inverse)]
    #[kani::stub(digest4, stub_digest4)]
    #[kani::unwind(17)]
    fn round_trip_2_of_3() {
        check_round_trip(2, 3);
    }

    #[kani::proof]
    #[kani::stub(getrandom::fill, crate::kani_stubs::fill)]
    #[kani::stub(zeroize::optimization_barrier, crate::kani_stubs::optimization_barrier)]
    #[kani::stub(zeroize::volatile_set, crate::kani_stubs::volatile_set)]
    #[kani::stub(zeroize::volatile_write, crate::kani_stubs::volatile_write)]
    #[kani::stub(gf256::mul, gf256::direct::mul)]
    #[kani::stub(gf256::inverse, gf256::direct::inverse)]
    #[kani::stub(digest4, stub_digest4)]
    #[kani::unwind(17)]
    fn round_trip_2_of_2() {
        check_round_trip(2, 2);
    }

    /// Any threshold, any 1 to 3 shares of 16 bytes with any x-coordinates
    /// (duplicates included): `recover_secret` returns, it never panics.
    #[kani::proof]
    #[kani::stub(zeroize::optimization_barrier, crate::kani_stubs::optimization_barrier)]
    #[kani::stub(gf256::mul, gf256::direct::mul)]
    #[kani::stub(gf256::inverse, gf256::direct::inverse)]
    #[kani::stub(digest4, stub_digest4)]
    #[kani::unwind(17)]
    fn recover_secret_never_panics() {
        let threshold: u8 = kani::any();
        let count: usize = kani::any();
        kani::assume((1..=3).contains(&count));
        let shares: Vec<Point> = (0..count).map(|_| (kani::any(), Zeroizing::new(kani::any::<[u8; 16]>().to_vec()))).collect();
        let _ = recover_secret(threshold, &shares);
    }
}
