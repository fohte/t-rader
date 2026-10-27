use crate::calendar::repository::CalendarEventRepositoryError;
use chrono::{DateTime, NaiveDate, Utc};

const E_STAT_EXTERNAL_ID_PATTERN: &str =
    r"^(00200573|00350300|00100409|00550300|00100401|00200531):[0-9]{12}$";
const FRED_EXTERNAL_ID_PATTERN: &str = r"^(10|50|54|53|9):[0-9]{4}-[0-9]{2}-[0-9]{2}$";

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
    pub external_id_pattern: String,
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
            external_id_pattern: external_id_pattern.into(),
        }
    }

    pub fn for_event_source(event_source: &str) -> Option<Self> {
        let (country, external_id_pattern) = match event_source {
            "e_stat" => ("JP", E_STAT_EXTERNAL_ID_PATTERN),
            "fred" => ("US", FRED_EXTERNAL_ID_PATTERN),
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
