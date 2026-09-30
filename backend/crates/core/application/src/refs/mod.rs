mod error;
mod repository;
mod types;
mod use_cases;

pub use error::{RefRepositoryError, RefUseCaseError};
pub use repository::{RefRepository, SharedRefRepository};
pub use types::{
    IndicatorRef, RefKind, RefSearchMatch, RefTerm, ResolvedRef, SectorRef, StockRef, ThemeRef,
};
pub use use_cases::RefUseCases;

pub fn sanitize_like(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '%' | '_' | '\\'))
        .collect()
}

pub fn normalize_term(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    value.nfkc().collect::<String>().to_lowercase()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{normalize_term, sanitize_like};

    #[rstest]
    #[case::width_variants("ＤＥＭＯＫＥＹ", "demokey")]
    #[case::case_variants("DemoKey", "demokey")]
    #[case::already_normalized("架空語", "架空語")]
    fn normalize_term_unifies_width_and_case(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize_term(input), expected);
    }

    #[rstest]
    #[case::wildcard_metacharacters("demo%_\\key", "demokey")]
    #[case::plain_text("demo-key", "demo-key")]
    fn sanitize_like_removes_wildcard_metacharacters(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(sanitize_like(input), expected);
    }
}
