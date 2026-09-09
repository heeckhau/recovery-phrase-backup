mod codec;
mod crypto;
mod error;
mod gf256;
mod rs1024;
mod shamir;
mod wordlist;

use std::collections::{BTreeMap, BTreeSet};

use codec::ShareData;
pub use error::Error;

/// A group's member threshold and member count (1..=16 each). See `split`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupSpec {
    pub member_threshold: u8,
    pub member_count: u8,
}

fn validate_passphrase(passphrase: &str) -> Result<(), Error> {
    for c in passphrase.chars() {
        let code = c as u32;
        if !(32..=126).contains(&code) {
            return Err(Error::InvalidPassphrase(code));
        }
    }
    Ok(())
}

/// Splits `master_secret` into mnemonic shares across `groups`, `group_threshold`
/// of which are needed to recover it. Always encodes `ext = 1` (extendable),
/// per the spec's reference `GenerateShares`. `passphrase` may be empty —
/// SLIP-39 always encrypts regardless — but must be printable ASCII.
/// `iteration_exponent` (0..=15) sets the PBKDF2 cost to `2500 << e`; 0 is
/// the spec's default.
///
/// Returns one `Vec<String>` of member mnemonics per group, in the same
/// order as `groups`.
pub fn split(
    master_secret: &[u8; 32],
    group_threshold: u8,
    groups: &[GroupSpec],
    passphrase: &str,
    iteration_exponent: u8,
) -> Result<Vec<Vec<String>>, Error> {
    validate_passphrase(passphrase)?;

    if iteration_exponent > 15 {
        return Err(Error::InvalidIterationExponent(iteration_exponent));
    }

    let group_count = groups.len();
    if group_count == 0 || group_count > 16 {
        return Err(Error::InvalidGroupCount(group_count));
    }
    if group_threshold == 0 || group_threshold as usize > group_count {
        return Err(Error::InvalidGroupThreshold { group_threshold, group_count: group_count as u8 });
    }
    for (i, g) in groups.iter().enumerate() {
        let group_index = i as u8;
        if g.member_count == 0 || g.member_count > 16 {
            return Err(Error::InvalidMemberCount { group_index, count: g.member_count });
        }
        if g.member_threshold == 0 || g.member_threshold > g.member_count || (g.member_threshold == 1 && g.member_count > 1) {
            return Err(Error::InvalidMemberThreshold {
                group_index,
                member_threshold: g.member_threshold,
                member_count: g.member_count,
            });
        }
    }

    let mut id_bytes = [0u8; 2];
    getrandom::fill(&mut id_bytes)?;
    let id = u16::from_be_bytes(id_bytes) & 0x7FFF; // 15 bits
    const EXTENDABLE: bool = true;

    let ems = crypto::encrypt(master_secret, passphrase, iteration_exponent, id, EXTENDABLE);
    let group_shares = shamir::split_secret(group_threshold, group_count as u8, &ems)?;

    let mut result = Vec::with_capacity(group_count);
    for (group_index, group_secret) in group_shares {
        let spec = groups[group_index as usize];
        let member_shares = shamir::split_secret(spec.member_threshold, spec.member_count, &group_secret)?;

        let mut mnemonics = Vec::with_capacity(spec.member_count as usize);
        for (member_index, share_value) in member_shares {
            let share = ShareData {
                id,
                extendable: EXTENDABLE,
                iteration_exponent,
                group_index,
                group_threshold,
                group_count: group_count as u8,
                member_index,
                member_threshold: spec.member_threshold,
                share_value,
            };
            mnemonics.push(codec::encode(&share).join(" "));
        }
        result.push(mnemonics);
    }

    Ok(result)
}

/// Recombines a flat, order-independent list of mnemonic shares (any mix
/// of groups) into the original 32-byte master secret. Reads `id`/`ext`/`e`
/// from the shares themselves, so shares from other SLIP-39
/// implementations (e.g. a Trezor, which may use `ext = 0`) work too.
///
/// A wrong `passphrase` is **not** detected here: SLIP-39's decryption is
/// intentionally unauthenticated (a plausible-deniability property), so a
/// wrong passphrase silently returns a different, wrong 32 bytes rather
/// than an error. `Ok` only means the shares were self-consistent.
pub fn combine(mnemonics: &[&str], passphrase: &str) -> Result<[u8; 32], Error> {
    validate_passphrase(passphrase)?;

    if mnemonics.is_empty() {
        return Err(Error::NoShares);
    }

    let shares: Vec<ShareData> =
        mnemonics.iter().map(|m| codec::decode(&m.split_whitespace().collect::<Vec<_>>())).collect::<Result<_, _>>()?;

    let first = shares[0].clone();
    for share in &shares[1..] {
        if share.id != first.id {
            return Err(Error::MismatchedShares { field: "identifier" });
        }
        if share.extendable != first.extendable {
            return Err(Error::MismatchedShares { field: "extendable flag" });
        }
        if share.iteration_exponent != first.iteration_exponent {
            return Err(Error::MismatchedShares { field: "iteration exponent" });
        }
        if share.group_threshold != first.group_threshold {
            return Err(Error::MismatchedShares { field: "group threshold" });
        }
        if share.group_count != first.group_count {
            return Err(Error::MismatchedShares { field: "group count" });
        }
        if share.share_value.len() != first.share_value.len() {
            return Err(Error::MismatchedShares { field: "share length" });
        }
    }

    if first.share_value.len() != 32 {
        return Err(Error::UnsupportedSecretLength(first.share_value.len() * 8));
    }

    let mut groups: BTreeMap<u8, Vec<ShareData>> = BTreeMap::new();
    for share in shares {
        groups.entry(share.group_index).or_default().push(share);
    }

    let group_threshold = first.group_threshold;
    if groups.len() as u8 != group_threshold {
        return Err(Error::WrongGroupCount { have: groups.len() as u8, need: group_threshold });
    }

    let mut group_points: Vec<(u8, Vec<u8>)> = Vec::with_capacity(groups.len());
    for (group_index, members) in groups {
        let member_threshold = members[0].member_threshold;
        for m in &members[1..] {
            if m.member_threshold != member_threshold {
                return Err(Error::MismatchedMemberThreshold { group_index });
            }
        }

        let mut seen_indices = BTreeSet::new();
        for m in &members {
            if !seen_indices.insert(m.member_index) {
                return Err(Error::DuplicateMemberIndex { group_index, member_index: m.member_index });
            }
        }

        if members.len() as u8 != member_threshold {
            return Err(Error::WrongMemberShareCount { group_index, have: members.len() as u8, need: member_threshold });
        }

        let member_points: Vec<(u8, Vec<u8>)> = members.into_iter().map(|m| (m.member_index, m.share_value)).collect();
        let group_secret = shamir::recover_secret(member_threshold, &member_points)?;
        group_points.push((group_index, group_secret));
    }

    let ems = shamir::recover_secret(group_threshold, &group_points)?;
    let ms = crypto::decrypt(&ems, passphrase, first.iteration_exponent, first.id, first.extendable);

    Ok(ms.try_into().expect("already validated share length is 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> [u8; 32] {
        let mut s = [0u8; 32];
        for (i, b) in s.iter_mut().enumerate() {
            *b = i as u8;
        }
        s
    }

    fn single_group(member_threshold: u8, member_count: u8) -> Vec<GroupSpec> {
        vec![GroupSpec { member_threshold, member_count }]
    }

    #[test]
    fn split_then_combine_round_trips_single_group() {
        let ms = secret();
        let groups = split(&ms, 1, &single_group(2, 3), "TREZOR", 0).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);

        let chosen: Vec<&str> = groups[0][..2].iter().map(String::as_str).collect();
        assert_eq!(combine(&chosen, "TREZOR").unwrap(), ms);
    }

    #[test]
    fn split_then_combine_round_trips_multi_group() {
        // Alice's example from the spec: 2 groups of {2-of-3 friends, could
        // be any two of the configured groups}.
        let ms = secret();
        let groups_spec = vec![
            GroupSpec { member_threshold: 1, member_count: 1 },
            GroupSpec { member_threshold: 3, member_count: 5 },
            GroupSpec { member_threshold: 2, member_count: 6 },
        ];
        let shares = split(&ms, 2, &groups_spec, "", 0).unwrap();

        // Use group 0's single share plus 3-of-5 from group 1.
        let mut chosen: Vec<&str> = vec![shares[0][0].as_str()];
        chosen.extend(shares[1][..3].iter().map(String::as_str));
        assert_eq!(combine(&chosen, "").unwrap(), ms);
    }

    #[test]
    fn wrong_passphrase_returns_a_different_secret_not_an_error() {
        let ms = secret();
        let groups = split(&ms, 1, &single_group(2, 3), "right", 0).unwrap();
        let chosen: Vec<&str> = groups[0][..2].iter().map(String::as_str).collect();
        let recovered = combine(&chosen, "wrong").unwrap();
        assert_ne!(recovered, ms);
    }

    #[test]
    fn rejects_invalid_group_count() {
        let ms = secret();
        assert_eq!(split(&ms, 1, &[], "", 0), Err(Error::InvalidGroupCount(0)));
        let too_many = vec![GroupSpec { member_threshold: 1, member_count: 1 }; 17];
        assert_eq!(split(&ms, 1, &too_many, "", 0), Err(Error::InvalidGroupCount(17)));
    }

    #[test]
    fn rejects_invalid_group_threshold() {
        let ms = secret();
        assert_eq!(
            split(&ms, 0, &single_group(1, 1), "", 0),
            Err(Error::InvalidGroupThreshold { group_threshold: 0, group_count: 1 })
        );
        assert_eq!(
            split(&ms, 2, &single_group(1, 1), "", 0),
            Err(Error::InvalidGroupThreshold { group_threshold: 2, group_count: 1 })
        );
    }

    #[test]
    fn rejects_invalid_member_count_and_threshold() {
        let ms = secret();
        assert_eq!(
            split(&ms, 1, &single_group(1, 0), "", 0),
            Err(Error::InvalidMemberCount { group_index: 0, count: 0 })
        );
        assert_eq!(
            split(&ms, 1, &single_group(1, 3), "", 0),
            Err(Error::InvalidMemberThreshold { group_index: 0, member_threshold: 1, member_count: 3 }),
            "threshold 1 requires exactly 1 member"
        );
        assert_eq!(
            split(&ms, 1, &single_group(4, 3), "", 0),
            Err(Error::InvalidMemberThreshold { group_index: 0, member_threshold: 4, member_count: 3 })
        );
    }

    #[test]
    fn rejects_invalid_iteration_exponent() {
        let ms = secret();
        assert_eq!(split(&ms, 1, &single_group(1, 1), "", 16), Err(Error::InvalidIterationExponent(16)));
    }

    #[test]
    fn rejects_non_ascii_passphrase() {
        let ms = secret();
        assert_eq!(split(&ms, 1, &single_group(1, 1), "café", 0), Err(Error::InvalidPassphrase('é' as u32)));
        assert_eq!(combine(&["x"], "café"), Err(Error::InvalidPassphrase('é' as u32)));
    }

    #[test]
    fn rejects_shares_from_different_backups() {
        let ms = secret();
        let a = split(&ms, 1, &single_group(2, 3), "", 0).unwrap();
        let b = split(&ms, 1, &single_group(2, 3), "", 0).unwrap(); // different random id
        let mixed = [a[0][0].as_str(), b[0][1].as_str()];
        assert_eq!(combine(&mixed, ""), Err(Error::MismatchedShares { field: "identifier" }));
    }

    #[test]
    fn rejects_wrong_group_count() {
        let ms = secret();
        let groups_spec = vec![GroupSpec { member_threshold: 1, member_count: 1 }, GroupSpec { member_threshold: 1, member_count: 1 }];
        let shares = split(&ms, 2, &groups_spec, "", 0).unwrap(); // needs both groups
        let only_one = [shares[0][0].as_str()];
        assert_eq!(combine(&only_one, ""), Err(Error::WrongGroupCount { have: 1, need: 2 }));
    }

    #[test]
    fn rejects_duplicate_member_share() {
        let ms = secret();
        let shares = split(&ms, 1, &single_group(2, 3), "", 0).unwrap();
        let duplicated = [shares[0][0].as_str(), shares[0][0].as_str()];
        assert_eq!(
            combine(&duplicated, ""),
            Err(Error::DuplicateMemberIndex { group_index: 0, member_index: 0 })
        );
    }

    #[test]
    fn rejects_too_few_member_shares() {
        let ms = secret();
        let shares = split(&ms, 1, &single_group(3, 5), "", 0).unwrap();
        let too_few = [shares[0][0].as_str(), shares[0][1].as_str()];
        assert_eq!(combine(&too_few, ""), Err(Error::WrongMemberShareCount { group_index: 0, have: 2, need: 3 }));
    }

    #[test]
    fn rejects_corrupted_share() {
        let ms = secret();
        let shares = split(&ms, 1, &single_group(2, 3), "", 0).unwrap();
        let mut words: Vec<String> = shares[0][0].split_whitespace().map(str::to_string).collect();
        let last = words.len() - 1;
        words[last] = if words[last] == "zero" { "academic".into() } else { "zero".into() };
        let tampered = words.join(" ");
        let chosen = [tampered.as_str(), shares[0][1].as_str()];
        assert_eq!(combine(&chosen, ""), Err(Error::InvalidChecksum));
    }

    #[test]
    fn rejects_shares_from_a_non_256_bit_backup() {
        // Official vectors.json "1. Valid mnemonic without sharing (128 bits)".
        let mnemonic = "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision keyboard";
        assert_eq!(combine(&[mnemonic], "TREZOR"), Err(Error::UnsupportedSecretLength(128)));
    }
}
