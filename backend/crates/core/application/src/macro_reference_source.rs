use chrono::NaiveDate;
use core_domain::calendar_event::CalendarEventReferenceKind;
use thiserror::Error;

use crate::calendar::macro_reference_targets::MacroReferenceTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroOfficialReference {
    pub target: MacroReferenceTarget,
    pub kind: CalendarEventReferenceKind,
    pub source: String,
    pub external_id: String,
    pub title: String,
    pub url: Option<String>,
    pub target_period_start: Option<NaiveDate>,
    pub target_period_end: Option<NaiveDate>,
    pub target_period_type: Option<String>,
    pub publication_evidence: Option<String>,
    pub publication_evidence_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferenceFetchResult {
    pub target: MacroReferenceTarget,
    pub references: Vec<MacroOfficialReference>,
    pub failure: Option<String>,
}

impl MacroReferenceFetchResult {
    pub fn success(target: MacroReferenceTarget, references: Vec<MacroOfficialReference>) -> Self {
        Self {
            target,
            references,
            failure: None,
        }
    }

    pub fn retry(target: MacroReferenceTarget, failure: impl Into<String>) -> Self {
        Self {
            target,
            references: Vec::new(),
            failure: Some(failure.into()),
        }
    }
}

#[derive(Debug, Error)]
#[error("official macroeconomic release lookup failed: {message}")]
pub struct MacroReferenceSourceError {
    message: String,
}

impl MacroReferenceSourceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait::async_trait]
pub trait MacroReferenceSource: Send + Sync {
    fn supports(&self, target: &MacroReferenceTarget) -> bool;

    async fn fetch_references(
        &self,
        targets: &[MacroReferenceTarget],
    ) -> Result<Vec<MacroReferenceFetchResult>, MacroReferenceSourceError>;
}

pub type SharedMacroReferenceSource = std::sync::Arc<dyn MacroReferenceSource>;
