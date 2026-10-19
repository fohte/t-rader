use unicode_normalization::UnicodeNormalization;

pub fn nfkc_ascii_lowercase(value: &str) -> String {
    value
        .nfkc()
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::nfkc_ascii_lowercase;

    #[rstest]
    #[case::ascii("Sample", "sample")]
    #[case::compatibility_characters("ＳＡＭＰＬＥ", "sample")]
    fn normalizes_for_comparison(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(nfkc_ascii_lowercase(input), expected);
    }
}
