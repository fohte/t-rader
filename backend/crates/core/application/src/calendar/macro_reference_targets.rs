use crate::calendar::repository::CalendarEventRepositoryError;
use chrono::{DateTime, NaiveDate, Utc};

const E_STAT_EXTERNAL_ID_PATTERN: &str =
    r"^(00200573|00350300|00100409|00550300|00100401|00200531):[0-9]{12}$";
const FRED_EXTERNAL_ID_PATTERN: &str = r"^(10|50|54|53|9):[0-9]{4}-[0-9]{2}-[0-9]{2}$";
const BOJ_EXTERNAL_ID_PATTERN: &str = r"^(meeting|tankan)-[0-9]{4}-[0-9]{2}-[0-9]{2}$";
const CENTRAL_BANK_DATE_EXTERNAL_ID_PATTERN: &str = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroReferenceTarget {
    pub event_source: String,
    pub event_external_id: String,
    pub event_title: String,
    pub same_day_event_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferenceTargetSelector {
    pub event_source: String,
    pub country: String,
    pub category: String,
    pub external_id_pattern: String,
    pub include_future: bool,
}

impl MacroReferenceTargetSelector {
    pub fn new(
        event_source: impl Into<String>,
        country: impl Into<String>,
        external_id_pattern: impl Into<String>,
    ) -> Self {
        Self {
            event_source: event_source.into(),
            country: country.into(),
            category: "indicator".to_owned(),
            external_id_pattern: external_id_pattern.into(),
            include_future: false,
        }
    }

    fn central_bank(event_source: &str, country: &str, pattern: &str) -> Self {
        Self {
            event_source: event_source.to_owned(),
            country: country.to_owned(),
            category: "central_bank".to_owned(),
            external_id_pattern: pattern.to_owned(),
            include_future: true,
        }
    }

    pub fn for_event_source(event_source: &str) -> Option<Self> {
        let (country, external_id_pattern) = match event_source {
            "e_stat" => ("JP", E_STAT_EXTERNAL_ID_PATTERN),
            "fred" => ("US", FRED_EXTERNAL_ID_PATTERN),
            "boj" => return Some(Self::central_bank("boj", "JP", BOJ_EXTERNAL_ID_PATTERN)),
            "fed" => {
                return Some(Self::central_bank(
                    "fed",
                    "US",
                    CENTRAL_BANK_DATE_EXTERNAL_ID_PATTERN,
                ));
            }
            "ecb" => {
                return Some(Self::central_bank(
                    "ecb",
                    "EU",
                    CENTRAL_BANK_DATE_EXTERNAL_ID_PATTERN,
                ));
            }
            _ => return None,
        };
        Some(Self::new(event_source, country, external_id_pattern))
    }
}

#[async_trait::async_trait]
pub trait MacroReferenceTargetRepository: Send + Sync {
    async fn list_pending_macro_reference_targets(
        &self,
        today: NaiveDate,
        selector: &MacroReferenceTargetSelector,
    ) -> Result<Vec<MacroReferenceTarget>, CalendarEventRepositoryError>;

    async fn mark_macro_reference_targets_checked(
        &self,
        targets: &[MacroReferenceTarget],
        checked_at: DateTime<Utc>,
    ) -> Result<(), CalendarEventRepositoryError>;
}

pub type SharedMacroReferenceTargetRepository = std::sync::Arc<dyn MacroReferenceTargetRepository>;

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::MacroReferenceTargetSelector;

    #[rstest]
    #[case::boj(
        "boj",
        Some(MacroReferenceTargetSelector {
            event_source: "boj".to_owned(),
            country: "JP".to_owned(),
            category: "central_bank".to_owned(),
            external_id_pattern: r"^(meeting|tankan)-[0-9]{4}-[0-9]{2}-[0-9]{2}$".to_owned(),
            include_future: true,
        })
    )]
    #[case::fed(
        "fed",
        Some(MacroReferenceTargetSelector {
            event_source: "fed".to_owned(),
            country: "US".to_owned(),
            category: "central_bank".to_owned(),
            external_id_pattern: r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$".to_owned(),
            include_future: true,
        })
    )]
    #[case::ecb(
        "ecb",
        Some(MacroReferenceTargetSelector {
            event_source: "ecb".to_owned(),
            country: "EU".to_owned(),
            category: "central_bank".to_owned(),
            external_id_pattern: r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$".to_owned(),
            include_future: true,
        })
    )]
    #[case::unsupported("unknown", None)]
    fn central_bank_sources_include_future_calendar_events(
        #[case] event_source: &str,
        #[case] expected: Option<MacroReferenceTargetSelector>,
    ) {
        assert_eq!(
            MacroReferenceTargetSelector::for_event_source(event_source),
            expected
        );
    }

    #[test]
    fn custom_selectors_remain_indicator_and_past_only() {
        assert_eq!(
            MacroReferenceTargetSelector::new("demo_source", "ZZ", r"^demo-[0-9]+$"),
            MacroReferenceTargetSelector {
                event_source: "demo_source".to_owned(),
                country: "ZZ".to_owned(),
                category: "indicator".to_owned(),
                external_id_pattern: r"^demo-[0-9]+$".to_owned(),
                include_future: false,
            },
        );
    }
}
