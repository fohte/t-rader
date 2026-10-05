use std::sync::Arc;

use core_application::{
    calendar::source::SharedCalendarEventSource, news_content::SharedNewsContentFetcher,
};
use gateway_boj::BojClient;
use gateway_ecb::EcbClient;
use gateway_fed::FedClient;
use gateway_firecrawl::FirecrawlClient;

use super::StartupError;

pub(crate) struct NewsAndCentralBankSources {
    pub(crate) news_content_fetcher: Option<SharedNewsContentFetcher>,
    pub(crate) boj_calendar_source: Option<SharedCalendarEventSource>,
    pub(crate) ecb_calendar_source: Option<SharedCalendarEventSource>,
    pub(crate) fed_calendar_source: Option<SharedCalendarEventSource>,
}

pub(crate) fn initialize_news_and_central_bank_sources(
    redis_url: &str,
) -> Result<NewsAndCentralBankSources, StartupError> {
    let news_content_fetcher: Option<SharedNewsContentFetcher> =
        match std::env::var("FIRECRAWL_API_KEY") {
            Ok(api_key) if !api_key.is_empty() => {
                let client = FirecrawlClient::new(redis_url, api_key).map_err(|error| {
                    StartupError::Config(format!("failed to initialize Firecrawl client: {error}"))
                })?;
                tracing::info!("Firecrawl news content source initialized");
                Some(Arc::new(client))
            }
            _ => {
                tracing::warn!(
                    "FIRECRAWL_API_KEY が未設定のため、ニュース本文の取得を起動しません"
                );
                None
            }
        };

    let boj_calendar_source: Option<SharedCalendarEventSource> =
        Some(Arc::new(BojClient::new().map_err(|error| {
            StartupError::Config(format!("failed to initialize BOJ calendar source: {error}"))
        })?));
    let ecb_calendar_source: Option<SharedCalendarEventSource> =
        Some(Arc::new(EcbClient::new().map_err(|error| {
            StartupError::Config(format!("failed to initialize ECB calendar source: {error}"))
        })?));
    let fed_calendar_source: Option<SharedCalendarEventSource> =
        Some(Arc::new(FedClient::new().map_err(|error| {
            StartupError::Config(format!(
                "failed to initialize Federal Reserve calendar source: {error}"
            ))
        })?));

    Ok(NewsAndCentralBankSources {
        news_content_fetcher,
        boj_calendar_source,
        ecb_calendar_source,
        fed_calendar_source,
    })
}
