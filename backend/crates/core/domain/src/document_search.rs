use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDocumentSearch {
    pub normalized_text: String,
    pub terms: Vec<String>,
}

pub fn prepare_document_search(text: &str) -> PreparedDocumentSearch {
    let normalized_text = text.nfkc().flat_map(char::to_lowercase).collect::<String>();
    let terms = normalized_text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .flat_map(|segment| {
            let characters = segment.chars().collect::<Vec<_>>();
            if characters.len() == 1 {
                return vec![segment.to_owned()];
            }
            characters
                .windows(2)
                .map(|pair| pair.iter().collect::<String>())
                .collect()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    PreparedDocumentSearch {
        normalized_text,
        terms,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{PreparedDocumentSearch, prepare_document_search};

    #[rstest]
    #[case::japanese("ＡＢＣと日本語", "abcと日本語", vec!["ab", "bc", "cと", "と日", "日本", "本語"])]
    #[case::case_and_spacing("Alpha beta", "alpha beta", vec!["al", "be", "et", "ha", "lp", "ph", "ta"])]
    #[case::one_character("語", "語", vec!["語"])]
    #[case::punctuation_only("!?", "!?", vec![])]
    fn normalizes_text_and_creates_distinct_bigrams(
        #[case] input: &str,
        #[case] normalized_text: &str,
        #[case] terms: Vec<&str>,
    ) {
        assert_eq!(
            prepare_document_search(input),
            PreparedDocumentSearch {
                normalized_text: normalized_text.into(),
                terms: terms.into_iter().map(str::to_owned).collect(),
            },
        );
    }
}
