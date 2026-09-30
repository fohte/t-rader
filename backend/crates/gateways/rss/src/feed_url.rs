use core_application::rss_feed::RssFeedUrlValidator;
use reqwest::Url;

#[derive(Clone, Copy, Debug, Default)]
pub struct HttpRssFeedUrlValidator;

impl RssFeedUrlValidator for HttpRssFeedUrlValidator {
    fn is_valid_http_url(&self, url: &str) -> bool {
        Url::parse(url).is_ok_and(|parsed| matches!(parsed.scheme(), "http" | "https"))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::HttpRssFeedUrlValidator;
    use core_application::rss_feed::RssFeedUrlValidator;

    #[rstest]
    #[case::http("http://feeds.example.invalid/markets.xml", true)]
    #[case::https("https://feeds.example.invalid/markets.xml", true)]
    #[case::ftp("ftp://feeds.example.invalid/markets.xml", false)]
    #[case::missing_scheme("feeds.example.invalid/markets.xml", false)]
    #[case::malformed("not-a-url", false)]
    fn accepts_only_valid_http_urls(#[case] url: &str, #[case] expected: bool) {
        let validator = HttpRssFeedUrlValidator;

        assert_eq!(validator.is_valid_http_url(url), expected);
    }
}
