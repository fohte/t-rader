use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::web_search::{WebSearchTimeRange, WebSearchTopic};

pub const SEARCH_DEPTH: &str = "basic";
pub const SEARCH_MAX_RESULTS: u8 = 5;
pub const SEARCH_INCLUDE_PUBLISHED_DATE: bool = true;
pub const SEARCH_INCLUDE_USAGE: bool = true;
pub const EXTRACT_DEPTH: &str = "basic";
pub const EXTRACT_FORMAT: &str = "markdown";
pub const EXTRACT_INCLUDE_USAGE: bool = true;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TavilyCredentialGeneration([u8; 32]);

impl TavilyCredentialGeneration {
    pub fn from_secret(secret: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        update_field(&mut hasher, "purpose", b"tavily-credential-generation-v1");
        update_field(&mut hasher, "secret", secret);
        Self(hasher.finalize().into())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TavilyCredentialGeneration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TavilyCredentialGeneration([configured])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TavilyRetrievalCacheKind {
    Search,
    Extract,
}

impl TavilyRetrievalCacheKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Extract => "extract",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TavilyRetrievalCacheKey {
    digest: [u8; 32],
    kind: TavilyRetrievalCacheKind,
}

impl TavilyRetrievalCacheKey {
    pub fn search(
        authorization_scope: &str,
        account_id: &str,
        credential_generation: TavilyCredentialGeneration,
        query: &str,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Self {
        let mut hasher = Sha256::new();
        update_common_fields(
            &mut hasher,
            TavilyRetrievalCacheKind::Search,
            authorization_scope,
            account_id,
            credential_generation,
        );
        update_field(&mut hasher, "query", query.as_bytes());
        update_field(&mut hasher, "search_depth", SEARCH_DEPTH.as_bytes());
        update_field(&mut hasher, "max_results", &[SEARCH_MAX_RESULTS]);
        update_optional(&mut hasher, "topic", topic.map(WebSearchTopic::as_str));
        update_optional(
            &mut hasher,
            "time_range",
            time_range.map(WebSearchTimeRange::as_str),
        );
        update_field(
            &mut hasher,
            "include_published_date",
            &[u8::from(SEARCH_INCLUDE_PUBLISHED_DATE)],
        );
        update_field(
            &mut hasher,
            "include_usage",
            &[u8::from(SEARCH_INCLUDE_USAGE)],
        );
        Self {
            digest: hasher.finalize().into(),
            kind: TavilyRetrievalCacheKind::Search,
        }
    }

    pub fn extract(
        authorization_scope: &str,
        account_id: &str,
        credential_generation: TavilyCredentialGeneration,
        url: &str,
    ) -> Self {
        let mut hasher = Sha256::new();
        update_common_fields(
            &mut hasher,
            TavilyRetrievalCacheKind::Extract,
            authorization_scope,
            account_id,
            credential_generation,
        );
        update_field(&mut hasher, "urls_count", &[1]);
        update_field(&mut hasher, "url", url.as_bytes());
        update_field(&mut hasher, "extract_depth", EXTRACT_DEPTH.as_bytes());
        update_field(&mut hasher, "format", EXTRACT_FORMAT.as_bytes());
        update_field(
            &mut hasher,
            "include_usage",
            &[u8::from(EXTRACT_INCLUDE_USAGE)],
        );
        Self {
            digest: hasher.finalize().into(),
            kind: TavilyRetrievalCacheKind::Extract,
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.digest
    }

    pub const fn kind(&self) -> TavilyRetrievalCacheKind {
        self.kind
    }
}

impl std::fmt::Debug for TavilyRetrievalCacheKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TavilyRetrievalCacheKey")
            .field("kind", &self.kind)
            .field("digest", &"[redacted]")
            .finish()
    }
}

fn update_common_fields(
    hasher: &mut Sha256,
    kind: TavilyRetrievalCacheKind,
    authorization_scope: &str,
    account_id: &str,
    credential_generation: TavilyCredentialGeneration,
) {
    update_field(hasher, "key_version", b"tavily-retrieval-cache-v1");
    update_field(hasher, "kind", kind.as_str().as_bytes());
    update_field(
        hasher,
        "authorization_scope",
        authorization_scope.as_bytes(),
    );
    update_field(hasher, "account_id", account_id.as_bytes());
    update_field(
        hasher,
        "credential_generation",
        credential_generation.as_bytes(),
    );
}

fn update_optional(hasher: &mut Sha256, name: &str, value: Option<&str>) {
    match value {
        Some(value) => {
            update_field(hasher, name, &[1]);
            update_field(hasher, name, value.as_bytes());
        }
        None => update_field(hasher, name, &[0]),
    }
}

fn update_field(hasher: &mut Sha256, name: &str, value: &[u8]) {
    hasher.update((name.len() as u64).to_be_bytes());
    hasher.update(name.as_bytes());
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TavilyRetrievalCacheEntry {
    pub payload: Value,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TavilyRetrievalCacheWrite {
    pub payload: Value,
    pub fetched_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TavilyRetrievalCacheError {
    #[error("retrieval cache operation failed")]
    Storage,
    #[error("retrieval cache transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait TavilyRetrievalCacheLease: Send + Sync {
    async fn store(
        self: Box<Self>,
        entry: TavilyRetrievalCacheWrite,
    ) -> Result<(), TavilyRetrievalCacheError>;

    async fn release(self: Box<Self>) -> Result<(), TavilyRetrievalCacheError>;
}

pub enum TavilyRetrievalCacheLookup {
    Hit(TavilyRetrievalCacheEntry),
    Miss(Box<dyn TavilyRetrievalCacheLease>),
}

#[async_trait]
pub trait TavilyRetrievalCacheRepository: Send + Sync {
    async fn acquire(
        &self,
        key: TavilyRetrievalCacheKey,
        now: DateTime<Utc>,
    ) -> Result<TavilyRetrievalCacheLookup, TavilyRetrievalCacheError>;
}

pub type SharedTavilyRetrievalCacheRepository =
    Arc<dyn TavilyRetrievalCacheRepository + Send + Sync>;

#[cfg(test)]
mod tests {
    use crate::web_search::{WebSearchTimeRange, WebSearchTopic};

    use super::{TavilyCredentialGeneration, TavilyRetrievalCacheKey};

    #[test]
    fn search_keys_separate_every_request_and_authorization_input() {
        let credential = TavilyCredentialGeneration::from_secret(b"synthetic-key-a");
        let base = TavilyRetrievalCacheKey::search(
            "synthetic-scope-a",
            "synthetic-account",
            credential,
            "synthetic query",
            None,
            None,
        );

        assert_eq!(
            [
                base != TavilyRetrievalCacheKey::search(
                    "synthetic-scope-a",
                    "synthetic-account",
                    credential,
                    "synthetic query changed",
                    None,
                    None,
                ),
                base != TavilyRetrievalCacheKey::search(
                    "synthetic-scope-a",
                    "synthetic-account",
                    credential,
                    "synthetic query",
                    Some(WebSearchTopic::News),
                    None,
                ),
                base != TavilyRetrievalCacheKey::search(
                    "synthetic-scope-a",
                    "synthetic-account",
                    credential,
                    "synthetic query",
                    None,
                    Some(WebSearchTimeRange::Week),
                ),
                base != TavilyRetrievalCacheKey::search(
                    "synthetic-scope-b",
                    "synthetic-account",
                    credential,
                    "synthetic query",
                    None,
                    None,
                ),
                base != TavilyRetrievalCacheKey::search(
                    "synthetic-scope-a",
                    "synthetic-account",
                    TavilyCredentialGeneration::from_secret(b"synthetic-key-b"),
                    "synthetic query",
                    None,
                    None,
                ),
                base != TavilyRetrievalCacheKey::search(
                    "synthetic-scope-a",
                    "synthetic-account-changed",
                    credential,
                    "synthetic query",
                    None,
                    None,
                ),
            ],
            [true, true, true, true, true, true],
        );
    }

    #[test]
    fn extract_keys_keep_the_url_query_string() {
        let credential = TavilyCredentialGeneration::from_secret(b"synthetic-key");
        let base = TavilyRetrievalCacheKey::extract(
            "synthetic-scope-a",
            "synthetic-account",
            credential,
            "https://example.invalid/page?mode=one",
        );

        assert_eq!(
            [
                base != TavilyRetrievalCacheKey::extract(
                    "synthetic-scope-a",
                    "synthetic-account",
                    credential,
                    "https://example.invalid/page",
                ),
                base != TavilyRetrievalCacheKey::extract(
                    "synthetic-scope-b",
                    "synthetic-account",
                    credential,
                    "https://example.invalid/page?mode=one",
                ),
                base != TavilyRetrievalCacheKey::extract(
                    "synthetic-scope-a",
                    "synthetic-account",
                    TavilyCredentialGeneration::from_secret(b"synthetic-key-b"),
                    "https://example.invalid/page?mode=one",
                ),
                base != TavilyRetrievalCacheKey::extract(
                    "synthetic-scope-a",
                    "synthetic-account-b",
                    credential,
                    "https://example.invalid/page?mode=one",
                ),
            ],
            [true, true, true, true],
        );
    }

    #[test]
    fn credential_generation_debug_output_does_not_contain_the_secret() {
        let generation = TavilyCredentialGeneration::from_secret(b"synthetic-secret");

        assert!(!format!("{generation:?}").contains("synthetic-secret"));
    }
}
