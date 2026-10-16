use std::time::{Duration, Instant};

use async_trait::async_trait;

#[async_trait]
pub(super) trait BarsIngestClock: Send + Sync {
    fn instant(&self) -> Instant;

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

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
