use core_application::rss_feed::RssFeedUrlValidator;
use reqwest::Url;

#[derive(Clone, Copy, Debug, Default)]
pub struct HttpRssFeedUrlValidator;

impl RssFeedUrlValidator for HttpRssFeedUrlValidator {
    fn is_valid_http_url(&self, url: &str) -> bool {
        Url::parse(url).is_ok_and(|parsed| matches!(parsed.scheme(), "http" | "https"))
    }
}
