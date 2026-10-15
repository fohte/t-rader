use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use core_domain::business_day::latest_business_day;
use futures_util::{FutureExt, StreamExt, stream};
use serde_json::json;

use super::TriggerUseCases;
use crate::agent_task_client::AgentTaskClient;
use crate::strategy_task::{SubmittedTask, TaskSource};
use crate::trigger::schedule::{parse_schedule, should_fire_cron_trigger};
use crate::trigger::{Trigger, TriggerUseCaseError};

const MAX_CONCURRENT_FIRES: usize = 8;
const MAX_BUSINESS_DAY_SEARCH_DAYS: usize = 366 * 28 + 7;

impl TriggerUseCases {
    pub async fn next_cron_fire_at(
        &self,
        now: DateTime<Utc>,
        recovery_window: Duration,
    ) -> Result<Option<DateTime<Utc>>, TriggerUseCaseError> {
        let triggers = self.repository.list_enabled_cron().await?;

        Ok(triggers
            .into_iter()
            .filter_map(|trigger| {
                let Some(expression) = trigger.schedule.as_deref() else {
                    tracing::warn!(trigger_id = %trigger.trigger_id, "cron trigger has no schedule; skip");
                    return None;
                };
                let schedule = match parse_schedule(expression) {
                    Ok(schedule) => schedule,
                    Err(error) => {
                        tracing::warn!(
                            trigger_id = %trigger.trigger_id,
                            schedule = expression,
                            error = %error,
                            "failed to parse cron schedule; skip",
                        );
                        return None;
                    }
                };
                next_fire_at(
                    &schedule,
                    trigger.last_fired_at.map(|value| value.with_timezone(&Utc)),
                    now,
                    recovery_window,
                    trigger.business_days_only,
                )
            })
            .min())
    }

    pub async fn run_cron_tick(
        &self,
        agent_client: &dyn AgentTaskClient,
        interval: Duration,
    ) -> usize {
        let triggers = match self.repository.list_enabled_cron().await {
            Ok(triggers) => triggers,
            Err(error) => {
                tracing::warn!(error = %error, "failed to list cron triggers");
                return 0;
            }
        };

        let now = Utc::now();
        let targets: Vec<Trigger> = triggers
            .into_iter()
            .filter(|trigger| {
                let Some(expression) = trigger.schedule.as_deref() else {
                    tracing::warn!(trigger_id = %trigger.trigger_id, "cron trigger has no schedule; skip");
                    return false;
                };
                let schedule = match parse_schedule(expression) {
                    Ok(schedule) => schedule,
                    Err(error) => {
                        tracing::warn!(
                            trigger_id = %trigger.trigger_id,
                            schedule = expression,
                            error = %error,
                            "failed to parse cron schedule; skip",
                        );
                        return false;
                    }
                };
                should_fire_cron_trigger(
                    &schedule,
                    trigger.last_fired_at.map(|value| value.with_timezone(&Utc)),
                    now,
                    interval,
                    trigger.business_days_only,
                )
            })
            .collect();

        let attempted = targets.len();
        let results = stream::iter(targets)
            .map(|trigger| async move {
                let trigger_id = trigger.trigger_id;
                std::panic::AssertUnwindSafe(self.claim_and_submit_cron(agent_client, trigger))
                    .catch_unwind()
                    .await
                    .map(|result| (trigger_id, result))
                    .map_err(|_| trigger_id)
            })
            .buffer_unordered(MAX_CONCURRENT_FIRES)
            .collect::<Vec<_>>()
            .await;
        for result in results {
            match result {
                Ok((_, Ok(_))) => {}
                Ok((trigger_id, Err(error))) => {
                    tracing::warn!(error = %error, trigger_id = %trigger_id, "cron trigger fire failed");
                }
                Err(trigger_id) => {
                    tracing::error!(trigger_id = %trigger_id, "cron trigger worker task panicked");
                }
            }
        }
        attempted
    }

    async fn claim_and_submit_cron(
        &self,
        agent_client: &dyn AgentTaskClient,
        trigger: Trigger,
    ) -> Result<Option<SubmittedTask>, TriggerUseCaseError> {
        let trigger_id = trigger.trigger_id;
        let (strategy_id, prompt) = self
            .resolve_trigger_prompt(&trigger, &json!({}), Utc::now())
            .await?;
        let prepared = self
            .strategy_tasks
            .prepare_submission(
                strategy_id,
                &prompt,
                TaskSource::Cron,
                trigger.purpose,
                None,
            )
            .await?;
        let fired_at = Utc::now().fixed_offset();
        let transaction = self.unit_of_work.begin().await?;
        if !self
            .repository
            .claim_cron_fire(
                &transaction,
                trigger_id,
                trigger.last_fired_at,
                trigger.updated_at,
                fired_at,
            )
            .await?
        {
            return Ok(None);
        }
        self.strategy_tasks
            .insert_prepared_submission(&transaction, &prepared)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        self.strategy_tasks
            .submit_prepared_submission(agent_client, prepared)
            .await
            .map(Some)
            .map_err(Into::into)
    }
}

fn next_fire_at(
    schedule: &cron::Schedule,
    last_fired_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    recovery_window: Duration,
    business_days_only: bool,
) -> Option<DateTime<Utc>> {
    if should_fire_cron_trigger(
        schedule,
        last_fired_at,
        now,
        recovery_window,
        business_days_only,
    ) {
        Some(now)
    } else {
        next_scheduled_time(schedule, now, business_days_only)
    }
}

fn next_scheduled_time(
    schedule: &cron::Schedule,
    now: DateTime<Utc>,
    business_days_only: bool,
) -> Option<DateTime<Utc>> {
    let mut after = now;
    let search_until = now + chrono::Duration::days(MAX_BUSINESS_DAY_SEARCH_DAYS as i64);
    for _ in 0..MAX_BUSINESS_DAY_SEARCH_DAYS {
        let due = schedule.after(&after).next()?;
        if due > search_until {
            return None;
        }
        if !business_days_only || is_business_day(due) {
            return Some(due);
        }

        let next_date = due
            .with_timezone(&chrono_tz::Asia::Tokyo)
            .date_naive()
            .succ_opt()?;
        let next_day_start = chrono_tz::Asia::Tokyo
            .from_local_datetime(&next_date.and_hms_opt(0, 0, 0)?)
            .single()?
            .with_timezone(&Utc);
        after = next_day_start - chrono::Duration::nanoseconds(1);
    }
    None
}

fn is_business_day(instant: DateTime<Utc>) -> bool {
    let date = instant.with_timezone(&chrono_tz::Asia::Tokyo).date_naive();
    latest_business_day(date) == date
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use rstest::rstest;

    use super::next_fire_at;
    use crate::trigger::schedule::parse_schedule;

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid test timestamp")
            .with_timezone(&Utc)
    }

    #[rstest]
    #[case::next_slot(
        "0 * * * *",
        None,
        "2026-01-02T12:34:00Z",
        Some("2026-01-02T13:00:00Z")
    )]
    #[case::missed_slot(
        "0 * * * *",
        Some("2026-01-02T11:00:00Z"),
        "2026-01-02T12:34:00Z",
        Some("2026-01-02T12:34:00Z")
    )]
    #[case::already_fired(
        "0 * * * *",
        Some("2026-01-02T12:00:00Z"),
        "2026-01-02T12:34:00Z",
        Some("2026-01-02T13:00:00Z")
    )]
    fn computes_the_next_fire_time(
        #[case] expression: &str,
        #[case] last_fired_at: Option<&str>,
        #[case] now: &str,
        #[case] expected: Option<&str>,
    ) {
        let actual = next_fire_at(
            &parse_schedule(expression).expect("valid schedule"),
            last_fired_at.map(timestamp),
            timestamp(now),
            std::time::Duration::from_secs(60),
            false,
        );
        let expected = expected.map(timestamp);

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::skips_holiday("0 15 * * *", "2025-02-10T15:00:00Z", Some("2025-02-11T15:00:00Z"))]
    #[case::weekend_only_schedule_has_no_business_day("0 9 * * 6", "2026-01-03T00:00:00Z", None)]
    fn finds_next_business_day_fire_time(
        #[case] expression: &str,
        #[case] now: &str,
        #[case] expected: Option<&str>,
    ) {
        let actual = next_fire_at(
            &parse_schedule(expression).expect("valid schedule"),
            None,
            timestamp(now),
            std::time::Duration::from_secs(60),
            true,
        );

        assert_eq!(actual, expected.map(timestamp));
    }
}
