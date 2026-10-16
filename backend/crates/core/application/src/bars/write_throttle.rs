use std::time::Duration;
use std::time::Instant;

use super::clock::BarsIngestClock;
use super::types::UsStockBarsIngestStats;

const PAGE_COOLDOWN_MULTIPLIER: u32 = 3;
const COMMIT_LATENCY_TARGET: Duration = Duration::from_millis(100);

#[derive(Default)]
pub(super) struct WriteThrottle {
    next_write_at: Option<Duration>,
}

impl WriteThrottle {
    pub(super) async fn wait_before_write(
        &mut self,
        clock: &dyn BarsIngestClock,
        deadline: Instant,
        stats: &mut UsStockBarsIngestStats,
    ) -> bool {
        let Some(next_write_at) = self.next_write_at.take() else {
            return clock.instant() < deadline;
        };
        let sleep_started_at = clock.now();
        let sleep_started_at_instant = clock.instant();
        if sleep_started_at_instant >= deadline {
            return false;
        }

        let delay = next_write_at.saturating_sub(sleep_started_at);
        let remaining = deadline.saturating_duration_since(sleep_started_at_instant);
        let sleep_duration = delay.min(remaining);

        if !sleep_duration.is_zero() {
            clock.sleep(sleep_duration).await;
            let waited = clock.now().saturating_sub(sleep_started_at);
            let waited_ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX);
            stats.write_throttle_wait_ms = stats.write_throttle_wait_ms.saturating_add(waited_ms);
        }

        clock.instant() < deadline
    }

    pub(super) fn record_page(
        &mut self,
        completed_at: Duration,
        write_duration: Duration,
        commit_duration: Duration,
    ) {
        self.next_write_at =
            Some(completed_at.saturating_add(page_cooldown(write_duration, commit_duration)));
    }
}

fn page_cooldown(write_duration: Duration, commit_duration: Duration) -> Duration {
    let base_delay = write_duration.saturating_mul(PAGE_COOLDOWN_MULTIPLIER);
    let slow_commit_extension = if commit_duration > COMMIT_LATENCY_TARGET {
        commit_duration
            .saturating_sub(COMMIT_LATENCY_TARGET)
            .saturating_mul(PAGE_COOLDOWN_MULTIPLIER)
    } else {
        Duration::ZERO
    };
    base_delay.saturating_add(slow_commit_extension)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use rstest::rstest;

    use super::{BarsIngestClock, WriteThrottle};
    use crate::bars::UsStockBarsIngestStats;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct ClockState {
        now: Duration,
        sleeps: Vec<Duration>,
    }

    struct FakeClock {
        started_at: Instant,
        state: Mutex<ClockState>,
    }

    impl Default for FakeClock {
        fn default() -> Self {
            Self {
                started_at: Instant::now(),
                state: Mutex::default(),
            }
        }
    }

    impl FakeClock {
        fn set_now(&self, now: Duration) {
            *self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = ClockState {
                now,
                ..ClockState::default()
            };
        }

        fn snapshot(&self) -> ClockState {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .to_owned()
        }
    }

    #[async_trait::async_trait]
    impl BarsIngestClock for FakeClock {
        fn now(&self) -> Duration {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .now
        }

        fn instant(&self) -> Instant {
            self.started_at + self.snapshot().now
        }

        async fn sleep(&self, duration: Duration) {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.sleeps.push(duration);
            state.now += duration;
        }
    }

    #[rstest]
    #[case::commit_below_target(100, 99, 300)]
    #[case::commit_at_target(100, 100, 300)]
    #[case::commit_above_target(250, 150, 900)]
    #[tokio::test]
    async fn waits_three_times_the_write_duration_and_extends_for_slow_commits(
        #[case] write_ms: u64,
        #[case] commit_ms: u64,
        #[case] expected_wait_ms: u64,
    ) {
        let clock = FakeClock::default();
        let mut stats = UsStockBarsIngestStats::default();
        let mut throttle = WriteThrottle::default();
        throttle.record_page(
            Duration::ZERO,
            Duration::from_millis(write_ms),
            Duration::from_millis(commit_ms),
        );
        let deadline = clock.instant() + Duration::from_secs(10);
        let can_write = throttle
            .wait_before_write(&clock, deadline, &mut stats)
            .await;

        assert_eq!(
            (can_write, clock.snapshot(), stats.write_throttle_wait_ms),
            (
                true,
                ClockState {
                    now: Duration::from_millis(expected_wait_ms),
                    sleeps: vec![Duration::from_millis(expected_wait_ms)],
                },
                expected_wait_ms,
            ),
        );
    }

    #[tokio::test]
    async fn sleeps_only_for_the_time_remaining_after_fetching_the_next_page() {
        let clock = FakeClock::default();
        clock.set_now(Duration::from_millis(900));
        let mut stats = UsStockBarsIngestStats::default();
        let mut throttle = WriteThrottle::default();
        throttle.record_page(
            Duration::from_millis(400),
            Duration::from_millis(400),
            Duration::from_millis(150),
        );
        let deadline = clock.instant() + Duration::from_secs(10);
        let can_write = throttle
            .wait_before_write(&clock, deadline, &mut stats)
            .await;

        assert_eq!(
            (can_write, clock.snapshot(), stats.write_throttle_wait_ms),
            (
                true,
                ClockState {
                    now: Duration::from_millis(1750),
                    sleeps: vec![Duration::from_millis(850)],
                },
                850,
            ),
        );
    }

    #[tokio::test]
    async fn stops_waiting_when_the_run_deadline_is_reached() {
        let clock = FakeClock::default();
        let mut stats = UsStockBarsIngestStats::default();
        let mut throttle = WriteThrottle::default();
        throttle.record_page(
            Duration::ZERO,
            Duration::from_millis(300),
            Duration::from_millis(100),
        );
        let deadline = clock.instant() + Duration::from_millis(200);

        let can_write = throttle
            .wait_before_write(&clock, deadline, &mut stats)
            .await;

        assert_eq!(
            (can_write, clock.snapshot(), stats.write_throttle_wait_ms),
            (
                false,
                ClockState {
                    now: Duration::from_millis(200),
                    sleeps: vec![Duration::from_millis(200)],
                },
                200,
            )
        );
    }
}
