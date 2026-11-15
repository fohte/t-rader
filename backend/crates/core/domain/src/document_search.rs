use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDocumentSearch {
    pub normalized_text: String,
    pub phrases: Vec<String>,
    pub terms: Vec<String>,
}

pub fn prepare_document_search(text: &str) -> PreparedDocumentSearch {
    let normalized_text = text
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let phrases = normalized_text
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let terms = normalized_text
        .split_whitespace()
        .flat_map(|segment| {
            let characters = segment.chars().collect::<Vec<_>>();
            characters
                .windows(2)
                .map(|pair| pair.iter().collect::<String>())
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    PreparedDocumentSearch {
        normalized_text,
        phrases,
        terms,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{PreparedDocumentSearch, prepare_document_search};

    #[rstest]
    #[case::japanese("ＡＢＣと日本語", "abcと日本語", vec!["abcと日本語"], vec!["ab", "bc", "cと", "と日", "日本", "本語"])]
    #[case::whitespace_collapsed("  Alpha\t beta  ", "alpha beta", vec!["alpha", "beta"], vec!["al", "be", "et", "ha", "lp", "ph", "ta"])]
    #[case::one_character("語", "語", vec!["語"], vec![])]
    #[case::punctuation("!?", "!?", vec!["!?"], vec!["!?"])]
    #[case::empty(" \t\n", "", vec![], vec![])]
    fn normalizes_text_and_creates_distinct_bigrams(
        #[case] input: &str,
        #[case] normalized_text: &str,
        #[case] phrases: Vec<&str>,
        #[case] terms: Vec<&str>,
    ) {
        assert_eq!(
            prepare_document_search(input),
            PreparedDocumentSearch {
                normalized_text: normalized_text.into(),
                phrases: phrases.into_iter().map(str::to_owned).collect(),
                terms: terms.into_iter().map(str::to_owned).collect(),
            },
        );
    }
}
