// Bit-packing between a ShareData struct and its mnemonic word encoding.
// See the spec's "Format of the share mnemonic": a 40-bit header (id, ext,
// e, group index, group threshold, group count, member index, member
// threshold), the zero-padded share value, then a 30-bit RS1024 checksum,
// all as one bitstream chunked into 10-bit words.
//
// Bit-level read/write is delegated to `bitvec` (a mature, extremely
// widely used crate — not a crypto primitive itself, but exactly the kind
// of fiddly index arithmetic worth not hand-rolling: this file's padding
// math already had one subtle bug caught by the official test vectors).
use std::fmt;
use std::ops::{Deref, DerefMut};

use bitvec::prelude::*;
use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, MIN_WORD_COUNT};
use crate::rs1024;
use crate::wordlist;
use crate::SecretBytes;

#[derive(Clone, PartialEq, Eq)]
pub struct ShareData {
    pub id: u16, // 15-bit
    pub extendable: bool,
    pub iteration_exponent: u8, // 4-bit (0..16)
    pub group_index: u8,        // 4-bit (0..16)
    pub group_threshold: u8,    // actual value 1..=16 (encoded as value-1)
    pub group_count: u8,        // actual value 1..=16 (encoded as value-1)
    pub member_index: u8,       // 4-bit (0..16)
    pub member_threshold: u8,   // actual value 1..=16 (encoded as value-1)
    pub share_value: SecretBytes, // zeroized on drop
}

// Same convention as core's PrivateKey: never print the secret bytes.
impl fmt::Debug for ShareData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareData")
            .field("id", &self.id)
            .field("extendable", &self.extendable)
            .field("iteration_exponent", &self.iteration_exponent)
            .field("group_index", &self.group_index)
            .field("group_threshold", &self.group_threshold)
            .field("group_count", &self.group_count)
            .field("member_index", &self.member_index)
            .field("member_threshold", &self.member_threshold)
            .field("share_value", &format_args!("REDACTED ({} bytes)", self.share_value.len()))
            .finish()
    }
}

type Bits = BitVec<u8, Msb0>;

/// A bit buffer holding a share's full bitstream (which includes the
/// secret share value), scrubbed on drop. `bitvec` doesn't implement
/// `Zeroize`, so this wraps it to zero the underlying bytes.
struct SecretBits(Bits);

impl Drop for SecretBits {
    fn drop(&mut self) {
        self.0.as_raw_mut_slice().zeroize();
    }
}

impl Deref for SecretBits {
    type Target = Bits;
    fn deref(&self) -> &Bits {
        &self.0
    }
}

impl DerefMut for SecretBits {
    fn deref_mut(&mut self) -> &mut Bits {
        &mut self.0
    }
}

/// Appends the low `width` bits of `value`, big-endian.
fn push_uint(bits: &mut Bits, value: u32, width: usize) {
    if width == 0 {
        return;
    }
    let start = bits.len();
    bits.resize(start + width, false);
    bits[start..].store_be(value);
}

fn push_bytes(bits: &mut Bits, bytes: &[u8]) {
    for &b in bytes {
        push_uint(bits, b as u32, 8);
    }
}

/// Reads a `width`-bit (<=32) big-endian unsigned integer at `start`.
fn read_uint(bits: &Bits, start: usize, width: usize) -> u32 {
    if width == 0 {
        return 0;
    }
    bits[start..start + width].load_be()
}

fn words_from_bits(bits: &Bits) -> Vec<u16> {
    bits.chunks(10).map(|chunk| chunk.load_be()).collect()
}

fn bits_from_words(words: &[u16]) -> Bits {
    let mut bits = Bits::new();
    for &w in words {
        push_uint(&mut bits, w as u32, 10);
    }
    bits
}

const HEADER_BITS: usize = 15 + 1 + 4 + 4 + 4 + 4 + 4 + 4; // 40
const CHECKSUM_BITS: usize = 30;

/// Encodes `share` as its mnemonic word sequence.
pub fn encode(share: &ShareData) -> Vec<String> {
    let mut bits = SecretBits(Bits::new());
    push_uint(&mut bits, share.id as u32, 15);
    push_uint(&mut bits, share.extendable as u32, 1);
    push_uint(&mut bits, share.iteration_exponent as u32, 4);
    push_uint(&mut bits, share.group_index as u32, 4);
    push_uint(&mut bits, (share.group_threshold - 1) as u32, 4);
    push_uint(&mut bits, (share.group_count - 1) as u32, 4);
    push_uint(&mut bits, share.member_index as u32, 4);
    push_uint(&mut bits, (share.member_threshold - 1) as u32, 4);

    let value_bits = share.share_value.len() * 8;
    let padding = (10 - (value_bits % 10)) % 10;
    push_uint(&mut bits, 0, padding);
    push_bytes(&mut bits, &share.share_value);

    let data_words = Zeroizing::new(words_from_bits(&bits));
    let checksum = rs1024::create_checksum(share.extendable, &data_words);
    for word in checksum {
        push_uint(&mut bits, word as u32, 10);
    }

    Zeroizing::new(words_from_bits(&bits))
        .iter()
        .map(|&w| wordlist::word_at(w).expect("word index always in range").to_string())
        .collect()
}

/// Decodes a share's mnemonic words back into a `ShareData`, validating
/// the RS1024 checksum and padding.
pub fn decode(words: &[&str]) -> Result<ShareData, Error> {
    if words.len() < MIN_WORD_COUNT {
        return Err(Error::InvalidMnemonicWordCount(words.len()));
    }

    // The word indices *are* the share, so they're secret too. The error
    // path below reports the word's 1-indexed *position* rather than the
    // word itself — a mistyped word could plausibly be a near-miss on an
    // actual share word, so echoing it back is best avoided.
    let indices: Zeroizing<Vec<u16>> = Zeroizing::new(
        words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let lower = w.to_lowercase();
                wordlist::index_of(&lower).ok_or(Error::UnknownWord(i + 1))
            })
            .collect::<Result<_, _>>()?,
    );

    let bits = SecretBits(bits_from_words(&indices));
    let total_bits = bits.len();

    let extendable = read_uint(&bits, 15, 1) == 1;
    if !rs1024::verify_checksum(extendable, &indices) {
        return Err(Error::InvalidChecksum);
    }

    let ps_bits_total = total_bits - HEADER_BITS - CHECKSUM_BITS;
    // Per spec: padding length = (padded share value length) mod 16, and
    // MUST NOT exceed 8 bits.
    let padding = ps_bits_total % 16;
    if padding > 8 {
        return Err(Error::InvalidPadding);
    }
    if read_uint(&bits, HEADER_BITS, padding) != 0 {
        return Err(Error::InvalidPadding);
    }

    let n = (ps_bits_total - padding) / 8;
    if n * 8 < 128 {
        return Err(Error::ShareValueTooShort(n * 8));
    }

    let group_threshold = read_uint(&bits, 24, 4) as u8 + 1;
    let group_count = read_uint(&bits, 28, 4) as u8 + 1;
    if group_threshold > group_count {
        return Err(Error::InvalidGroupThreshold { group_threshold, group_count });
    }

    let share_value: SecretBytes =
        Zeroizing::new((0..n).map(|i| read_uint(&bits, HEADER_BITS + padding + i * 8, 8) as u8).collect());

    Ok(ShareData {
        id: read_uint(&bits, 0, 15) as u16,
        extendable,
        iteration_exponent: read_uint(&bits, 16, 4) as u8,
        group_index: read_uint(&bits, 20, 4) as u8,
        group_threshold,
        group_count,
        member_index: read_uint(&bits, 32, 4) as u8,
        member_threshold: read_uint(&bits, 36, 4) as u8 + 1,
        share_value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_share() -> ShareData {
        ShareData {
            id: 12345,
            extendable: true,
            iteration_exponent: 0,
            group_index: 2,
            group_threshold: 3,
            group_count: 5,
            member_index: 1,
            member_threshold: 4,
            share_value: Zeroizing::new((0u8..32).collect()),
        }
    }

    #[test]
    fn encode_then_decode_round_trips() {
        let share = sample_share();
        let words = encode(&share);
        assert_eq!(words.len(), 33); // 40 + 260 + 30 = 330 bits = 33 words for a 256-bit share value
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        let decoded = decode(&refs).unwrap();
        assert_eq!(decoded, share);
    }

    #[test]
    fn tolerates_autocapitalized_first_word() {
        let share = sample_share();
        let mut words = encode(&share);
        words[0] = {
            let mut c = words[0].chars();
            match c.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + c.as_str(),
                None => words[0].clone(),
            }
        };
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        assert_eq!(decode(&refs).unwrap(), share);
    }

    #[test]
    fn rejects_group_threshold_above_group_count() {
        // Well-formed bits with a valid checksum, but a group threshold no
        // group count can satisfy — the spec requires G >= GT.
        let share = ShareData { group_threshold: 4, group_count: 3, ..sample_share() };
        let words = encode(&share);
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        assert_eq!(decode(&refs), Err(Error::InvalidGroupThreshold { group_threshold: 4, group_count: 3 }));
    }

    #[test]
    fn rejects_unknown_word() {
        let share = sample_share();
        let mut words = encode(&share);
        words[0] = "not-a-real-word".to_string();
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        assert_eq!(decode(&refs), Err(Error::UnknownWord(1)));
    }

    #[test]
    fn rejects_bad_checksum() {
        let share = sample_share();
        let mut words = encode(&share);
        let last = words.len() - 1;
        let original_idx = wordlist::index_of(&words[last]).unwrap();
        let bumped = wordlist::word_at((original_idx + 1) % 1024).unwrap();
        words[last] = bumped.to_string();
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        assert_eq!(decode(&refs), Err(Error::InvalidChecksum));
    }

    #[test]
    fn rejects_too_few_words() {
        assert_eq!(decode(&["academic"; 5]), Err(Error::InvalidMnemonicWordCount(5)));
    }

    #[test]
    fn decodes_official_two_share_vector_consistently() {
        // Official vectors.json "4. Basic sharing 2-of-3 (128 bits)": two
        // member shares of the same single-group secret.
        const SHARE_A: &str = "shadow pistol academic always adequate wildlife fancy gross oasis cylinder mustang wrist rescue view short owner flip making coding armed";
        const SHARE_B: &str = "shadow pistol academic acid actress prayer class unknown daughter sweater depict flip twice unkind craft early superior advocate guest smoking";

        let a: Vec<&str> = SHARE_A.split_whitespace().collect();
        let b: Vec<&str> = SHARE_B.split_whitespace().collect();
        assert_eq!(a.len(), 20); // 128-bit share value: 40 + 130 + 30 = 200 bits = 20 words

        let share_a = decode(&a).unwrap();
        let share_b = decode(&b).unwrap();

        assert_eq!(share_a.share_value.len(), 16); // 128 bits
        // Shares of the same secret must agree on everything except which
        // member they are.
        assert_eq!(share_a.id, share_b.id);
        assert_eq!(share_a.extendable, share_b.extendable);
        assert_eq!(share_a.iteration_exponent, share_b.iteration_exponent);
        assert_eq!(share_a.group_index, share_b.group_index);
        assert_eq!(share_a.group_threshold, share_b.group_threshold);
        assert_eq!(share_a.group_count, share_b.group_count);
        assert_eq!(share_a.member_threshold, share_b.member_threshold);
        assert_eq!(share_a.member_threshold, 2); // "2-of-3" per the vector's description
        assert_ne!(share_a.member_index, share_b.member_index);
        assert_ne!(share_a.share_value, share_b.share_value);
    }

    #[test]
    fn all_fields_round_trip_across_their_ranges() {
        for group_threshold in 1..=16u8 {
            for member_threshold in 1..=16u8 {
                let share = ShareData {
                    id: 32767,
                    extendable: group_threshold % 2 == 0,
                    iteration_exponent: 15,
                    group_index: 15,
                    group_threshold,
                    group_count: 16,
                    member_index: 15,
                    member_threshold,
                    share_value: Zeroizing::new(vec![0xAB; 32]),
                };
                let words = encode(&share);
                let refs: Vec<&str> = words.iter().map(String::as_str).collect();
                assert_eq!(decode(&refs).unwrap(), share);
            }
        }
    }
}
