use std::future::Future;

use chrono::Weekday;
use graphile_worker::{
    Cron, Crontab, CrontabFill, CrontabTimer, CrontabTimerError, TaskHandler, Worker, WorkerOptions,
};
use sqlx::PgPool;

use crate::{
    jobs::{
        fred::FredIngest,
        ingest_run_recovery::IngestRunRecovery,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        prediction::PredictionGrading,
    },
    state::{SchedulerDependencies, SchedulerState},
};

const GRAPHILE_WORKER_SCHEMA: &str = "graphile_worker";
const JQUANTS_QUEUE: &str = "jquants";
const MAX_ATTEMPTS: u16 = 3;
const INGEST_RUN_RECOVERY_INTERVAL_MINUTES: u32 = 5;

pub struct Scheduler {
    worker: Worker,
}

impl Scheduler {
    pub async fn initialize(
        pool: PgPool,
        dependencies: SchedulerDependencies,
        shutdown_signal: impl Future<Output = ()> + Send + 'static,
    ) -> Result<Self, String> {
        let crontabs = build_crontabs(
            dependencies.fred_source.is_some(),
            dependencies.short_selling_source.is_some() && dependencies.margin_source.is_some(),
        )
        .map_err(|error| error.to_string())?;
        let state = SchedulerState { dependencies };
        let recovery_cron =
            Cron::every_n_minutes::<IngestRunRecovery>(INGEST_RUN_RECOVERY_INTERVAL_MINUTES)
                .map_err(|error| error.to_string())?
                .fill(CrontabFill::minutes(10));
        let worker = WorkerOptions::default()
            .pg_pool(pool)
            .schema(GRAPHILE_WORKER_SCHEMA)
            .concurrency(2)
            .use_notification_delivery(false)
            .use_local_time(false)
            .listen_os_shutdown_signals(false)
            .shutdown_signal(shutdown_signal)
            .add_extension(state)
            .define_job::<FredIngest>()
            .define_job::<IngestRunRecovery>()
            .define_job::<ShortRatioIngest>()
            .define_job::<ShortSaleReportIngest>()
            .define_job::<MarginIngest>()
            .define_job::<PredictionGrading>()
            .with_crons(crontabs)
            .with_cron(recovery_cron)
            .init()
            .await
            .map_err(|error| error.to_string())?;

        Ok(Self { worker })
    }

    pub async fn run(self) -> Result<(), String> {
        self.worker.run().await.map_err(|error| error.to_string())
    }
}

fn build_crontabs(
    include_fred: bool,
    include_jquants: bool,
) -> Result<Vec<Crontab>, CrontabTimerError> {
    let mut crontabs = Vec::new();
    if include_fred {
        crontabs.push(daily_cron::<FredIngest>("fred_ingest", 11, 30, None)?);
    }
    if include_jquants {
        crontabs.extend([
            daily_cron::<ShortRatioIngest>("short_ratio_ingest", 12, 0, Some(JQUANTS_QUEUE))?,
            daily_cron::<ShortSaleReportIngest>(
                "short_sale_report_ingest",
                12,
                15,
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<MarginIngest>("margin_ingest", 12, 30, Some(JQUANTS_QUEUE))?,
        ]);
    }
    crontabs.push(weekly_cron::<PredictionGrading>(
        "prediction_grading",
        Weekday::Sun,
        14,
        0,
    )?);
    Ok(crontabs)
}

fn configure_cron<T: TaskHandler>(
    timer: CrontabTimer,
    id: &str,
    fill: CrontabFill,
    queue: Option<&str>,
) -> Crontab {
    let mut crontab = Crontab::new(timer, T::IDENTIFIER);
    crontab.options.id = Some(id.to_string());
    crontab.options.fill = Some(fill);
    crontab.options.max = Some(MAX_ATTEMPTS);
    crontab.options.queue = queue.map(str::to_string);
    crontab
}

fn daily_cron<T: TaskHandler>(
    id: &str,
    utc_hour: u32,
    utc_minute: u32,
    queue: Option<&str>,
) -> Result<Crontab, CrontabTimerError> {
    Ok(configure_cron::<T>(
        CrontabTimer::daily_at(utc_hour, utc_minute)?,
        id,
        CrontabFill::days(3),
        queue,
    ))
}

fn weekly_cron<T: TaskHandler>(
    id: &str,
    weekday: Weekday,
    utc_hour: u32,
    utc_minute: u32,
) -> Result<Crontab, CrontabTimerError> {
    Ok(configure_cron::<T>(
        CrontabTimer::weekly_on(weekday, utc_hour, utc_minute)?,
        id,
        CrontabFill::weeks(2),
        None,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::Weekday;
    use graphile_worker::{Crontab, CrontabFill, CrontabTimer, TaskHandler};
    use rstest::rstest;

    use crate::jobs::{
        fred::FredIngest,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        prediction::PredictionGrading,
    };

    use super::{JQUANTS_QUEUE, build_crontabs, configure_cron};

    fn expected_cron<T: TaskHandler>(
        timer: Option<CrontabTimer>,
        id: &str,
        fill: CrontabFill,
        queue: Option<&str>,
    ) -> Option<Crontab> {
        Some(configure_cron::<T>(timer?, id, fill, queue))
    }

    #[test]
    fn schedules_fixed_jst_times_and_serializes_jquants_jobs() {
        let expected = [
            expected_cron::<FredIngest>(
                CrontabTimer::daily_at(11, 30).ok(),
                "fred_ingest",
                CrontabFill::days(3),
                None,
            ),
            expected_cron::<ShortRatioIngest>(
                CrontabTimer::daily_at(12, 0).ok(),
                "short_ratio_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<ShortSaleReportIngest>(
                CrontabTimer::daily_at(12, 15).ok(),
                "short_sale_report_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<MarginIngest>(
                CrontabTimer::daily_at(12, 30).ok(),
                "margin_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<PredictionGrading>(
                CrontabTimer::weekly_on(Weekday::Sun, 14, 0).ok(),
                "prediction_grading",
                CrontabFill::weeks(2),
                None,
            ),
        ]
        .into_iter()
        .collect::<Option<Vec<_>>>();

        assert_eq!(build_crontabs(true, true).ok(), expected);
    }

    #[test]
    fn configures_missed_tick_fill_retry_limit_and_jquants_queue() {
        let actual = build_crontabs(true, true).ok().map(|crontabs| {
            crontabs
                .into_iter()
                .map(|crontab| {
                    (
                        crontab.options.id,
                        crontab.options.fill,
                        crontab.options.max,
                        crontab.options.queue,
                    )
                })
                .collect::<Vec<_>>()
        });

        assert_eq!(
            actual,
            Some(vec![
                (
                    Some("fred_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some("short_ratio_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("short_sale_report_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("margin_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("prediction_grading".to_string()),
                    Some(CrontabFill::weeks(2)),
                    Some(3),
                    None,
                ),
            ]),
        );
    }

    #[rstest]
    #[case::neither_source(false, false, vec!["prediction_grading"])]
    #[case::fred_only(true, false, vec!["fred_ingest", "prediction_grading"])]
    #[case::jquants_only(false, true, vec!["short_ratio_ingest", "short_sale_report_ingest", "margin_ingest", "prediction_grading"])]
    fn schedules_only_configured_sources(
        #[case] include_fred: bool,
        #[case] include_jquants: bool,
        #[case] expected_ids: Vec<&str>,
    ) {
        let actual_ids = build_crontabs(include_fred, include_jquants)
            .ok()
            .map(|crontabs| {
                crontabs
                    .into_iter()
                    .filter_map(|crontab| crontab.options.id)
                    .collect::<Vec<_>>()
            });
        let expected_ids = Some(expected_ids.into_iter().map(str::to_string).collect());

        assert_eq!(actual_ids, expected_ids);
    }
}
