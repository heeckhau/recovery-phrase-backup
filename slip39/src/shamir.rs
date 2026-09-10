// GF(256) Shamir secret sharing: SplitSecret/RecoverSecret from the SLIP-39
// spec. Used identically at both the group level and the member level of
// the two-level scheme (see lib.rs).
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::Error;
use crate::gf256;
use crate::SecretBytes;

type HmacSha256 = Hmac<Sha256>;

/// A share point: its x-coordinate (share index) and secret y value-vector.
pub type Point = (u8, SecretBytes);

/// First 4 bytes of HMAC-SHA256(key, msg) — the "digest" used as the fixed
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
/// (x_i, y_i) points with pairwise distinct x_i.
fn interpolate(x: u8, points: &[Point]) -> SecretBytes {
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
            basis = gf256::mul(basis, gf256::div(num, den).expect("distinct x-coordinates never collide"));
        }
        for k in 0..n {
            result[k] = gf256::add(result[k], gf256::mul(basis, yi[k]));
        }
    }
    result
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
        shares.push((x, interpolate(x, &interpolation_set)));
    }

    Ok(shares)
}

/// RecoverSecret(T, shares): reconstructs the secret from a set of
/// share-index/share-value pairs, verifying the embedded digest. Returns
/// `Error::ShareSetInconsistent` if the shares don't reconstruct a
/// consistent secret — which covers wrong shares, too few shares, and
/// corrupted share data alike, since all of those break the digest check
/// the same way.
pub fn recover_secret(threshold: u8, shares: &[Point]) -> Result<SecretBytes, Error> {
    let Some(first) = shares.first() else {
        return Err(Error::ShareSetInconsistent);
    };

    if threshold == 1 {
        return Ok(first.1.clone());
    }

    let secret = interpolate(255, shares);
    let d = interpolate(254, shares);

    let r = &d[4..];
    let expected_prefix = digest4(r, &secret);

    if d[0..4] == expected_prefix {
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
    fn rejects_invalid_secret_length() {
        assert_eq!(split_secret(1, 2, &[0u8; 15]), Err(Error::InvalidSecretLength(120)));
        assert_eq!(split_secret(1, 2, &[0u8; 17]), Err(Error::InvalidSecretLength(136)));
    }
}
