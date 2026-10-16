use std::time::{Duration, Instant};

use async_trait::async_trait;

#[async_trait]
pub(super) trait BarsIngestClock: Send + Sync {
    fn now(&self) -> Duration;

    async fn sleep(&self, duration: Duration);
}

pub(super) struct SystemBarsIngestClock {
    started_at: Instant,
}

impl SystemBarsIngestClock {
    pub(super) fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

#[async_trait]
impl BarsIngestClock for SystemBarsIngestClock {
    fn now(&self) -> Duration {
        self.started_at.elapsed()
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
