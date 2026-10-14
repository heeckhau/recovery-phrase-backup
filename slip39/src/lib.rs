mod codec;
mod crypto;
mod error;
mod gf256;
mod rs1024;
mod shamir;
mod wordlist;

use std::collections::{BTreeMap, BTreeSet};

use zeroize::Zeroizing;

use codec::ShareData;
pub use error::Error;

/// The 1024-word SLIP-39 wordlist, in order — e.g. for autocomplete in a UI.
pub fn wordlist() -> &'static [&'static str] {
    wordlist::words()
}

/// Secret-bearing bytes (share values, group secrets, the encrypted and
/// plaintext master secret, Feistel state), scrubbed on drop.
pub(crate) type SecretBytes = Zeroizing<Vec<u8>>;

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
/// of which are needed to recover it. `master_secret` may be any length
/// SLIP-39 allows: at least 128 bits and a multiple of 16 bits (16, 20,
/// 24, 28 or 32 bytes for BIP39 entropy; 64 bytes for a BIP32 seed). Always
/// encodes `ext = 1` (extendable), per the spec's reference `GenerateShares`.
/// `passphrase` may be empty — SLIP-39 always encrypts regardless — but must
/// be printable ASCII. `iteration_exponent` (0..=15) sets the PBKDF2 cost to
/// `2500 << e` per Feistel round. The spec sets no default; the reference
/// implementation and other tools use 1.
///
/// Returns one `Vec<String>` of member mnemonics per group, in the same
/// order as `groups`.
pub fn split(
    master_secret: &[u8],
    group_threshold: u8,
    groups: &[GroupSpec],
    passphrase: &str,
    iteration_exponent: u8,
) -> Result<Vec<Vec<String>>, Error> {
    // Checked here, before encryption: the Feistel network splits the
    // secret into two equal halves and would silently mangle an odd length.
    let bits = master_secret.len() * 8;
    if bits < 128 || !bits.is_multiple_of(16) {
        return Err(Error::InvalidSecretLength(bits));
    }

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

/// The structural metadata a single mnemonic share reveals about itself —
/// which group/member it is and the thresholds it was split with. None of
/// this is sensitive (unlike the share's actual value): anyone holding the
/// mnemonic can already read it directly, `inspect` just decodes it for
/// display, e.g. so a UI can label a pasted share "Group 2, share 1 of 3"
/// without the user having to track that by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShareInfo {
    pub id: u16,
    pub extendable: bool,
    pub iteration_exponent: u8,
    pub group_index: u8,
    pub group_threshold: u8,
    pub group_count: u8,
    pub member_index: u8,
    pub member_threshold: u8,
}

/// Decodes a single mnemonic share's metadata (not its secret value),
/// validating its checksum. Useful to identify/label a share — e.g. before
/// enough shares are on hand to `combine` — without exposing anything
/// sensitive.
pub fn inspect(mnemonic: &str) -> Result<ShareInfo, Error> {
    let share = codec::decode(&mnemonic.split_whitespace().collect::<Vec<_>>())?;
    Ok(ShareInfo {
        id: share.id,
        extendable: share.extendable,
        iteration_exponent: share.iteration_exponent,
        group_index: share.group_index,
        group_threshold: share.group_threshold,
        group_count: share.group_count,
        member_index: share.member_index,
        member_threshold: share.member_threshold,
    })
}

/// Recombines a flat, order-independent list of mnemonic shares (any mix
/// of groups) into the original master secret, at whatever length it was
/// split with. Reads `id`/`ext`/`e` from the shares themselves, so shares
/// from other SLIP-39 implementations (e.g. a Trezor, which may use
/// `ext = 0`) work too.
///
/// More shares/groups than strictly needed is fine — surplus is ignored
/// deterministically (lowest group/member indices used first) rather than
/// rejected, since every share has already been checked to share the same
/// id/ext/iteration-exponent/group-threshold/count/length, so "extra"
/// input is already confirmed to be genuinely part of the *same* backup,
/// not a mixed-up foreign one.
///
/// A wrong `passphrase` is **not** detected here: SLIP-39's decryption is
/// intentionally unauthenticated (a plausible-deniability property), so a
/// wrong passphrase silently returns different, wrong bytes rather than an
/// error. `Ok` only means the shares were self-consistent.
pub fn combine(mnemonics: &[&str], passphrase: &str) -> Result<Zeroizing<Vec<u8>>, Error> {
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

    let mut groups: BTreeMap<u8, Vec<ShareData>> = BTreeMap::new();
    for share in shares {
        groups.entry(share.group_index).or_default().push(share);
    }

    let group_threshold = first.group_threshold;
    if (groups.len() as u8) < group_threshold {
        return Err(Error::NotEnoughGroups { have: groups.len() as u8, need: group_threshold });
    }

    // BTreeMap iterates in ascending group_index order, so stopping once
    // we have enough deterministically prefers the lowest-indexed groups
    // and simply never looks at surplus ones beyond that.
    let mut group_points: Vec<shamir::Point> = Vec::with_capacity(group_threshold as usize);
    for (group_index, mut members) in groups {
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

        if (members.len() as u8) < member_threshold {
            return Err(Error::NotEnoughMemberShares { group_index, have: members.len() as u8, need: member_threshold });
        }
        members.sort_by_key(|m| m.member_index);
        members.truncate(member_threshold as usize);

        let member_points: Vec<shamir::Point> = members.into_iter().map(|m| (m.member_index, m.share_value)).collect();
        let group_secret = shamir::recover_secret(member_threshold, &member_points)?;
        group_points.push((group_index, group_secret));

        if group_points.len() == group_threshold as usize {
            break;
        }
    }

    let ems = shamir::recover_secret(group_threshold, &group_points)?;
    Ok(crypto::decrypt(&ems, passphrase, first.iteration_exponent, first.id, first.extendable))
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
    fn inspect_reveals_group_and_member_position() {
        let ms = secret();
        let groups_spec = vec![GroupSpec { member_threshold: 2, member_count: 3 }, GroupSpec { member_threshold: 3, member_count: 4 }];
        let shares = split(&ms, 2, &groups_spec, "", 0).unwrap();

        let info = inspect(&shares[1][2]).unwrap(); // group 1 (0-indexed), 3rd member
        assert_eq!(info.group_index, 1);
        assert_eq!(info.group_threshold, 2);
        assert_eq!(info.group_count, 2);
        assert_eq!(info.member_index, 2);
        assert_eq!(info.member_threshold, 3);

        // Every share of the same backup reports the same id.
        let other = inspect(&shares[0][0]).unwrap();
        assert_eq!(other.id, info.id);
    }

    #[test]
    fn inspect_rejects_invalid_share_without_panicking() {
        assert!(inspect("not a real mnemonic").is_err());
    }

    #[test]
    fn split_then_combine_round_trips_single_group() {
        let ms = secret();
        let groups = split(&ms, 1, &single_group(2, 3), "TREZOR", 0).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);

        let chosen: Vec<&str> = groups[0][..2].iter().map(String::as_str).collect();
        assert_eq!(*combine(&chosen, "TREZOR").unwrap(), ms);
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
        assert_eq!(*combine(&chosen, "").unwrap(), ms);
    }

    #[test]
    fn wrong_passphrase_returns_a_different_secret_not_an_error() {
        let ms = secret();
        let groups = split(&ms, 1, &single_group(2, 3), "right", 0).unwrap();
        let chosen: Vec<&str> = groups[0][..2].iter().map(String::as_str).collect();
        let recovered = combine(&chosen, "wrong").unwrap();
        assert_ne!(*recovered, ms);
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
    fn rejects_not_enough_groups() {
        let ms = secret();
        let groups_spec = vec![GroupSpec { member_threshold: 1, member_count: 1 }, GroupSpec { member_threshold: 1, member_count: 1 }];
        let shares = split(&ms, 2, &groups_spec, "", 0).unwrap(); // needs both groups
        let only_one = [shares[0][0].as_str()];
        assert_eq!(combine(&only_one, ""), Err(Error::NotEnoughGroups { have: 1, need: 2 }));
    }

    #[test]
    fn ignores_surplus_groups_beyond_the_threshold() {
        // 3 groups configured, but only 2 are required — bringing all 3
        // group's worth of shares should still work, not error, since
        // that's still unambiguously the same backup (checked via
        // id/ext/e/group-threshold/count/length before this point).
        let ms = secret();
        let groups_spec = vec![
            GroupSpec { member_threshold: 1, member_count: 1 },
            GroupSpec { member_threshold: 1, member_count: 1 },
            GroupSpec { member_threshold: 1, member_count: 1 },
        ];
        let shares = split(&ms, 2, &groups_spec, "", 0).unwrap();
        let all_three = [shares[0][0].as_str(), shares[1][0].as_str(), shares[2][0].as_str()];
        assert_eq!(*combine(&all_three, "").unwrap(), ms);
    }

    #[test]
    fn ignores_surplus_member_shares_beyond_the_threshold() {
        // 2-of-3 group: bringing all 3 member shares should still work.
        let ms = secret();
        let shares = split(&ms, 1, &single_group(2, 3), "", 0).unwrap();
        let all_three: Vec<&str> = shares[0].iter().map(String::as_str).collect();
        assert_eq!(*combine(&all_three, "").unwrap(), ms);
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
        assert_eq!(combine(&too_few, ""), Err(Error::NotEnoughMemberShares { group_index: 0, have: 2, need: 3 }));
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
    fn recovers_a_128_bit_backup() {
        // Official vectors.json "1. Valid mnemonic without sharing (128 bits)".
        let mnemonic = "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision keyboard";
        let expected = [0xbb, 0x54, 0xaa, 0xc4, 0xb8, 0x9d, 0xc8, 0x68, 0xba, 0x37, 0xd9, 0xcc, 0x21, 0xb2, 0xce, 0xce];
        assert_eq!(*combine(&[mnemonic], "TREZOR").unwrap(), expected);
    }

    #[test]
    fn round_trips_every_bip39_length_and_a_bip32_seed() {
        // 12/15/18/21/24-word BIP39 entropy, and a 512-bit BIP32 seed (what
        // python-slip39 shares when backing up a seed rather than entropy).
        for len in [16, 20, 24, 28, 32, 64] {
            let ms: Vec<u8> = (0..len as u8).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect();
            let groups_spec = vec![GroupSpec { member_threshold: 2, member_count: 3 }, GroupSpec { member_threshold: 1, member_count: 1 }];
            let shares = split(&ms, 2, &groups_spec, "TREZOR", 0).unwrap();

            let chosen = [shares[0][0].as_str(), shares[0][2].as_str(), shares[1][0].as_str()];
            assert_eq!(*combine(&chosen, "TREZOR").unwrap(), ms, "{len}-byte secret");
        }
    }

    #[test]
    fn rejects_secret_lengths_slip39_does_not_allow() {
        let groups_spec = single_group(1, 1);
        assert_eq!(split(&[0u8; 14], 1, &groups_spec, "", 0), Err(Error::InvalidSecretLength(112)), "under 128 bits");
        assert_eq!(split(&[0u8; 17], 1, &groups_spec, "", 0), Err(Error::InvalidSecretLength(136)), "not a multiple of 16 bits");
        assert_eq!(split(&[0u8; 33], 1, &groups_spec, "", 0), Err(Error::InvalidSecretLength(264)), "odd byte count");
    }
}
