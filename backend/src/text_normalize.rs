//! 表記揺れ (全角/半角・大文字小文字) を吸収する文字列正規化。
//! ref_term (参照型の別名) 照合など、テキスト同士の緩い一致判定に使う。

use unicode_normalization::UnicodeNormalization;

/// 全角/半角・大文字小文字の表記揺れを吸収する正規化。lowercase だけでは
/// 全角/半角の幅統一が行われないため、NFKC を先に適用して統一する
pub fn normalize(s: &str) -> String {
    s.nfkc().collect::<String>().to_lowercase()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::normalize;

    #[rstest]
    #[case::zenkaku_to_hankaku("ＵＳＤＪＰＹ", "usdjpy")]
    #[case::uppercase_to_lowercase("USDJPY", "usdjpy")]
    #[case::already_normalized("トヨタ", "トヨタ")]
    fn normalize_unifies_width_and_case(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize(input), expected);
    }
}
