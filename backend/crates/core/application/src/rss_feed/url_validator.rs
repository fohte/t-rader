use std::sync::Arc;

pub trait RssFeedUrlValidator: Send + Sync {
    fn is_valid_http_url(&self, url: &str) -> bool;
}

pub type SharedRssFeedUrlValidator = Arc<dyn RssFeedUrlValidator>;
