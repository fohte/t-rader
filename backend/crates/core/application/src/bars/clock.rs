use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[async_trait]
pub(super) trait BarsIngestClock: Send + Sync {
    fn instant(&self) -> Instant;

    fn utc_now(&self) -> DateTime<Utc>;

    async fn sleep(&self, duration: Duration);
}

pub(super) struct SystemBarsIngestClock;

impl SystemBarsIngestClock {
    pub(super) fn new() -> Self {
        Self
    }
}

#[async_trait]
impl BarsIngestClock for SystemBarsIngestClock {
    fn instant(&self) -> Instant {
        Instant::now()
    }

    fn utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
