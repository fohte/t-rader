use std::time::Duration;

use super::clock::BarsIngestClock;
use super::types::UsStockBarsIngestStats;

const PAGE_COOLDOWN_MULTIPLIER: u32 = 3;
const COMMIT_LATENCY_TARGET: Duration = Duration::from_millis(100);

pub(super) async fn wait_after_page(
    clock: &dyn BarsIngestClock,
    write_duration: Duration,
    commit_duration: Duration,
    stats: &mut UsStockBarsIngestStats,
) {
    let delay = page_cooldown(write_duration, commit_duration);
    if delay.is_zero() {
        return;
    }

    let sleep_started_at = clock.now();
    clock.sleep(delay).await;
    let waited = clock.now().saturating_sub(sleep_started_at);
    let waited_ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX);
    stats.write_throttle_wait_ms = stats.write_throttle_wait_ms.saturating_add(waited_ms);
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
    use std::time::Duration;

    use rstest::rstest;

    use super::{BarsIngestClock, wait_after_page};
    use crate::bars::UsStockBarsIngestStats;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct ClockState {
        now: Duration,
        sleeps: Vec<Duration>,
    }

    #[derive(Default)]
    struct FakeClock {
        state: Mutex<ClockState>,
    }

    impl FakeClock {
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
        wait_after_page(
            &clock,
            Duration::from_millis(write_ms),
            Duration::from_millis(commit_ms),
            &mut stats,
        )
        .await;

        assert_eq!(
            (clock.snapshot(), stats.write_throttle_wait_ms),
            (
                ClockState {
                    now: Duration::from_millis(expected_wait_ms),
                    sleeps: vec![Duration::from_millis(expected_wait_ms)],
                },
                expected_wait_ms,
            ),
        );
    }

    #[tokio::test]
    async fn records_total_wait_and_returns_to_the_base_delay_when_commit_latency_recovers() {
        let clock = FakeClock::default();
        let mut stats = UsStockBarsIngestStats::default();
        wait_after_page(
            &clock,
            Duration::from_millis(250),
            Duration::from_millis(150),
            &mut stats,
        )
        .await;
        wait_after_page(
            &clock,
            Duration::from_millis(100),
            Duration::from_millis(99),
            &mut stats,
        )
        .await;

        assert_eq!(
            (clock.snapshot(), stats.write_throttle_wait_ms),
            (
                ClockState {
                    now: Duration::from_millis(1200),
                    sleeps: vec![Duration::from_millis(900), Duration::from_millis(300)],
                },
                1200,
            ),
        );
    }
}
