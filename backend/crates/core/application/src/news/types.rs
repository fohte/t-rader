use chrono::{DateTime, FixedOffset, NaiveDate};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregationStats {
    pub fetched: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsArticle {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub body_snippet: Option<String>,
    pub published_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchNewsQuery {
    pub keyword: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}
