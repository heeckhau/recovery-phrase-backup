// Direct port of the RS1024 reference code from the SLIP-39 spec
// (https://github.com/satoshilabs/slips/blob/master/slip-0039.md#checksum).

const GEN: [u32; 10] = [
    0xe0e040, 0x1c1c080, 0x3838100, 0x7070200, 0xe0e0009, 0x1c0c2412, 0x38086c24, 0x3090fc48,
    0x21b1f890, 0x3f3f120,
];

fn polymod(values: impl IntoIterator<Item = u16>) -> u32 {
    let mut chk: u32 = 1;
    for v in values {
        let b = chk >> 20;
        chk = ((chk & 0xfffff) << 10) ^ (v as u32);
        for (i, gen) in GEN.iter().enumerate() {
            if (b >> i) & 1 == 1 {
                chk ^= gen;
            }
        }
    }
    chk
}

fn customization_string(extendable: bool) -> &'static [u8] {
    if extendable {
        b"shamir_extendable"
    } else {
        b"shamir"
    }
}

fn full_sequence(extendable: bool, data: &[u16]) -> impl Iterator<Item = u16> + '_ {
    customization_string(extendable).iter().map(|&b| b as u16).chain(data.iter().copied())
}

/// Verifies the RS1024 checksum over a share's full data-part word
/// sequence, including its trailing 3 checksum words.
pub fn verify_checksum(extendable: bool, data_with_checksum: &[u16]) -> bool {
    polymod(full_sequence(extendable, data_with_checksum)) == 1
}

/// Computes the 3-word RS1024 checksum for a share's data-part words
/// (excluding the checksum itself).
pub fn create_checksum(extendable: bool, data: &[u16]) -> [u16; 3] {
    let polymod_val = polymod(full_sequence(extendable, data).chain([0, 0, 0])) ^ 1;
    [
        ((polymod_val >> 20) & 1023) as u16,
        ((polymod_val >> 10) & 1023) as u16,
        (polymod_val & 1023) as u16,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wordlist;

    fn words_to_indices(words: &str) -> Vec<u16> {
        words.split_whitespace().map(|w| wordlist::index_of(w).unwrap()).collect()
    }

    // Official SLIP-39 test vectors #1 (valid) and #2 (last word tampered,
    // "invalid checksum") from trezor/python-shamir-mnemonic's vectors.json,
    // empirically confirmed to use ext=0 ("shamir", not "shamir_extendable").
    const VALID: &str = "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision keyboard";
    const TAMPERED_CHECKSUM: &str = "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision kidney";

    #[test]
    fn verifies_official_valid_vector() {
        assert!(verify_checksum(false, &words_to_indices(VALID)));
    }

    #[test]
    fn rejects_official_tampered_checksum_vector() {
        assert!(!verify_checksum(false, &words_to_indices(TAMPERED_CHECKSUM)));
    }

    #[test]
    fn rejects_valid_vector_under_wrong_customization_string() {
        assert!(!verify_checksum(true, &words_to_indices(VALID)));
    }

    #[test]
    fn create_checksum_round_trips_with_verify() {
        let data: Vec<u16> = (0..30).collect();
        for &extendable in &[false, true] {
            let checksum = create_checksum(extendable, &data);
            let mut full = data.clone();
            full.extend_from_slice(&checksum);
            assert!(verify_checksum(extendable, &full));
        }
    }

    #[test]
    fn create_checksum_detects_single_word_mutation() {
        let data: Vec<u16> = (0..30).collect();
        let checksum = create_checksum(false, &data);
        let mut full = data.clone();
        full.extend_from_slice(&checksum);
        full[5] ^= 1; // flip one bit of one data word
        assert!(!verify_checksum(false, &full));
    }
}

// Kani proofs (`cargo kani -p rpb-slip39 -Z stubbing`), for every data
// word sequence of the shortest share (20 words: 17 data, 3 checksum).
#[cfg(kani)]
mod proofs {
    use super::*;

    const DATA_WORDS: usize = 17;

    fn any_words() -> [u16; DATA_WORDS] {
        let data: [u16; DATA_WORDS] = kani::any();
        for &w in &data {
            kani::assume(w < 1024);
        }
        data
    }

    fn with_checksum(extendable: bool, data: &[u16; DATA_WORDS]) -> [u16; DATA_WORDS + 3] {
        let mut full = [0u16; DATA_WORDS + 3];
        full[..DATA_WORDS].copy_from_slice(data);
        full[DATA_WORDS..].copy_from_slice(&create_checksum(extendable, data));
        full
    }

    #[kani::proof]
    #[kani::unwind(40)]
    fn created_checksum_verifies() {
        let extendable: bool = kani::any();
        let full = with_checksum(extendable, &any_words());
        for &w in &full {
            assert!(w < 1024);
        }
        assert!(verify_checksum(extendable, &full));
    }

    /// Any one wrong word, anywhere (checksum words included), is caught.
    #[kani::proof]
    #[kani::unwind(40)]
    fn any_single_wrong_word_fails_verification() {
        let extendable: bool = kani::any();
        let mut full = with_checksum(extendable, &any_words());
        let position: usize = kani::any();
        kani::assume(position < full.len());
        let wrong: u16 = kani::any();
        kani::assume(wrong < 1024 && wrong != full[position]);
        full[position] = wrong;
        assert!(!verify_checksum(extendable, &full));
    }
}
