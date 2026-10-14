use std::sync::LazyLock;

/// The SLIP-39 wordlist: 1024 English words, alphabetically sorted. Source:
/// <https://github.com/satoshilabs/slips/blob/master/slip-0039/wordlist.txt>
/// (satoshilabs/slips, CC-BY-SA-4.0 — data only, embedded verbatim).
const WORDLIST_TXT: &str = include_str!("wordlist.txt");

static WORDLIST: LazyLock<Vec<&'static str>> = LazyLock::new(|| WORDLIST_TXT.lines().collect());

/// Looks up the word at a given 10-bit index (0..1024).
/// All 1024 words, in order — e.g. for autocomplete in a UI.
pub fn words() -> &'static [&'static str] {
    &WORDLIST
}

pub fn word_at(index: u16) -> Option<&'static str> {
    WORDLIST.get(index as usize).copied()
}

/// Looks up a word's 10-bit index. The wordlist is alphabetically sorted,
/// so this is a binary search.
pub fn index_of(word: &str) -> Option<u16> {
    WORDLIST.binary_search(&word).ok().map(|i| i as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_exactly_1024_words() {
        assert_eq!(WORDLIST.len(), 1024);
    }

    #[test]
    fn is_alphabetically_sorted_with_no_duplicates() {
        let mut sorted = WORDLIST.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 1024, "expected no duplicates");
        assert_eq!(*WORDLIST, sorted, "expected the file to already be sorted");
    }

    #[test]
    fn word_at_and_index_of_round_trip() {
        for i in 0..1024u16 {
            let word = word_at(i).unwrap();
            assert_eq!(index_of(word), Some(i));
        }
    }

    #[test]
    fn unknown_word_returns_none() {
        assert_eq!(index_of("not-a-real-word"), None);
    }
}
