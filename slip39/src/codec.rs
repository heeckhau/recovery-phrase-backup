// Bit-packing between a ShareData struct and its mnemonic word encoding.
// See the spec's "Format of the share mnemonic": a 40-bit header (id, ext,
// e, group index, group threshold, group count, member index, member
// threshold), the zero-padded share value, then a 30-bit RS1024 checksum,
// all as one bitstream chunked into 10-bit words.
use crate::error::{Error, MIN_WORD_COUNT};
use crate::rs1024;
use crate::wordlist;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareData {
    pub id: u16, // 15-bit
    pub extendable: bool,
    pub iteration_exponent: u8, // 4-bit (0..16)
    pub group_index: u8,        // 4-bit (0..16)
    pub group_threshold: u8,    // actual value 1..=16 (encoded as value-1)
    pub group_count: u8,        // actual value 1..=16 (encoded as value-1)
    pub member_index: u8,       // 4-bit (0..16)
    pub member_threshold: u8,   // actual value 1..=16 (encoded as value-1)
    pub share_value: Vec<u8>,
}

/// A simple bit buffer — clarity over cleverness, since a share is at most
/// a few hundred bits.
struct BitVec(Vec<bool>);

impl BitVec {
    fn new() -> Self {
        BitVec(Vec::new())
    }

    fn push_uint(&mut self, value: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.0.push((value >> i) & 1 == 1);
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.push_uint(b as u32, 8);
        }
    }

    fn from_words(words: &[u16]) -> Self {
        let mut v = BitVec::new();
        for &w in words {
            v.push_uint(w as u32, 10);
        }
        v
    }

    fn to_words(&self) -> Vec<u16> {
        self.0
            .chunks(10)
            .map(|chunk| chunk.iter().fold(0u16, |acc, &bit| (acc << 1) | (bit as u16)))
            .collect()
    }

    fn read_uint(&self, start: usize, bits: usize) -> u32 {
        self.0[start..start + bits].iter().fold(0u32, |acc, &bit| (acc << 1) | (bit as u32))
    }

    fn read_bytes(&self, start: usize, byte_len: usize) -> Vec<u8> {
        (0..byte_len).map(|i| self.read_uint(start + i * 8, 8) as u8).collect()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

const HEADER_BITS: usize = 15 + 1 + 4 + 4 + 4 + 4 + 4 + 4; // 40
const CHECKSUM_BITS: usize = 30;

/// Encodes `share` as its mnemonic word sequence.
pub fn encode(share: &ShareData) -> Vec<String> {
    let mut bits = BitVec::new();
    bits.push_uint(share.id as u32, 15);
    bits.push_uint(share.extendable as u32, 1);
    bits.push_uint(share.iteration_exponent as u32, 4);
    bits.push_uint(share.group_index as u32, 4);
    bits.push_uint((share.group_threshold - 1) as u32, 4);
    bits.push_uint((share.group_count - 1) as u32, 4);
    bits.push_uint(share.member_index as u32, 4);
    bits.push_uint((share.member_threshold - 1) as u32, 4);

    let value_bits = share.share_value.len() * 8;
    let padding = (10 - (value_bits % 10)) % 10;
    bits.push_uint(0, padding as u32);
    bits.push_bytes(&share.share_value);

    let data_words = bits.to_words();
    let checksum = rs1024::create_checksum(share.extendable, &data_words);
    for word in checksum {
        bits.push_uint(word as u32, 10);
    }

    bits.to_words()
        .into_iter()
        .map(|w| wordlist::word_at(w).expect("word index always in range").to_string())
        .collect()
}

/// Decodes a share's mnemonic words back into a `ShareData`, validating
/// the RS1024 checksum and padding.
pub fn decode(words: &[&str]) -> Result<ShareData, Error> {
    if words.len() < MIN_WORD_COUNT {
        return Err(Error::InvalidMnemonicWordCount(words.len()));
    }

    let indices: Vec<u16> = words
        .iter()
        .map(|w| {
            let lower = w.to_lowercase();
            wordlist::index_of(&lower).ok_or(Error::UnknownWord(lower))
        })
        .collect::<Result<_, _>>()?;

    let bits = BitVec::from_words(&indices);
    let total_bits = bits.len();

    let extendable = bits.read_uint(15, 1) == 1;
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
    if bits.read_uint(HEADER_BITS, padding) != 0 {
        return Err(Error::InvalidPadding);
    }

    let n = (ps_bits_total - padding) / 8;
    if n * 8 < 128 {
        return Err(Error::ShareValueTooShort(n * 8));
    }

    let share_value = bits.read_bytes(HEADER_BITS + padding, n);

    Ok(ShareData {
        id: bits.read_uint(0, 15) as u16,
        extendable,
        iteration_exponent: bits.read_uint(16, 4) as u8,
        group_index: bits.read_uint(20, 4) as u8,
        group_threshold: bits.read_uint(24, 4) as u8 + 1,
        group_count: bits.read_uint(28, 4) as u8 + 1,
        member_index: bits.read_uint(32, 4) as u8,
        member_threshold: bits.read_uint(36, 4) as u8 + 1,
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
            share_value: (0u8..32).collect(),
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
    fn rejects_unknown_word() {
        let share = sample_share();
        let mut words = encode(&share);
        words[0] = "not-a-real-word".to_string();
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        assert_eq!(decode(&refs), Err(Error::UnknownWord("not-a-real-word".to_string())));
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
                    share_value: vec![0xAB; 32],
                };
                let words = encode(&share);
                let refs: Vec<&str> = words.iter().map(String::as_str).collect();
                assert_eq!(decode(&refs).unwrap(), share);
            }
        }
    }
}
