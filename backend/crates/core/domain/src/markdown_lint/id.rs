use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct LintIdOccurrences(HashMap<(String, String), usize>);

impl LintIdOccurrences {
    pub(crate) fn next(&mut self, rule_id: &str, key: &str) -> String {
        let count = self
            .0
            .entry((rule_id.to_string(), key.to_string()))
            .and_modify(|count| *count += 1)
            .or_insert(1);
        lint_id(rule_id, key, *count)
    }
}

pub(crate) fn lint_id(rule_id: &str, key: &str, occurrence: usize) -> String {
    format!("{rule_id}:{}:{occurrence}", percent_encode(key))
}

pub fn warning_id_group(id: &str) -> Option<&str> {
    let (group_id, occurrence) = id.rsplit_once(':')?;
    occurrence
        .parse::<usize>()
        .ok()
        .filter(|count| *count > 0)?;
    Some(group_id)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{LintIdOccurrences, lint_id, warning_id_group};

    #[rstest]
    #[case::ascii_key("price", "sample-key", 1, "price:sample-key:1")]
    #[case::utf8_key("price", "12,347円", 2, "price:12%2C347%E5%86%86:2")]
    fn builds_stable_ids(
        #[case] rule_id: &str,
        #[case] key: &str,
        #[case] occurrence: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(lint_id(rule_id, key, occurrence), expected);
    }

    #[test]
    fn counts_occurrences_by_rule_and_exact_key() {
        let mut occurrences = LintIdOccurrences::default();
        assert_eq!(
            (
                occurrences.next("price", "same"),
                occurrences.next("relative", "same"),
                occurrences.next("price", "same"),
            ),
            (
                "price:same:1".to_string(),
                "relative:same:1".to_string(),
                "price:same:2".to_string(),
            ),
        );
    }

    #[rstest]
    #[case::valid("price:sample-key:2", Some("price:sample-key"))]
    #[case::zero_count("price:sample-key:0", None)]
    #[case::missing_count("price:sample-key", None)]
    fn parses_warning_groups(#[case] id: &str, #[case] expected: Option<&str>) {
        assert_eq!(warning_id_group(id), expected);
    }
}
