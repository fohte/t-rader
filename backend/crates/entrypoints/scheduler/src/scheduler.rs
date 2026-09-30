use std::future::Future;

use chrono::Weekday;
use core_application::strategy_task::STRATEGY_TASK_RECONCILE_QUEUE_NAME;
use graphile_worker::{
    Crontab, CrontabFill, CrontabTimer, CrontabTimerError, TaskHandler, Worker, WorkerOptions,
};
use sqlx::PgPool;

use crate::{
    jobs::{
        fred::FredIngest,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        prediction::PredictionGrading,
        strategy_task_reconcile::StrategyTaskReconcile,
        trigger_evaluation::TriggerEvaluation,
    },
    state::{SchedulerDependencies, SchedulerState},
};

const GRAPHILE_WORKER_SCHEMA: &str = "graphile_worker";
const JQUANTS_QUEUE: &str = "jquants";
const MAX_ATTEMPTS: u16 = 3;

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
            dependencies.strategy_task_reconcile_enabled,
        )
        .map_err(|error| error.to_string())?;
        let state = SchedulerState { dependencies };
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
            .define_job::<ShortRatioIngest>()
            .define_job::<ShortSaleReportIngest>()
            .define_job::<MarginIngest>()
            .define_job::<PredictionGrading>()
            .define_job::<StrategyTaskReconcile>()
            .define_job::<TriggerEvaluation>()
            .with_crons(crontabs)
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
    include_strategy_task_reconcile: bool,
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
    if include_strategy_task_reconcile {
        crontabs.push(every_minute_cron::<StrategyTaskReconcile>(
            "strategy_task_reconcile",
            Some(STRATEGY_TASK_RECONCILE_QUEUE_NAME),
        ));
    }
    crontabs.push(every_minute_cron::<TriggerEvaluation>(
        "trigger_evaluation",
        None,
    ));
    Ok(crontabs)
}

fn configure_cron<T: TaskHandler>(
    timer: CrontabTimer,
    id: &str,
    fill: Option<CrontabFill>,
    queue: Option<&str>,
) -> Crontab {
    let mut crontab = Crontab::new(timer, T::IDENTIFIER);
    crontab.options.id = Some(id.to_string());
    crontab.options.fill = fill;
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
        Some(CrontabFill::days(3)),
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
        Some(CrontabFill::weeks(2)),
        None,
    ))
}

fn every_minute_cron<T: TaskHandler>(id: &str, queue: Option<&str>) -> Crontab {
    configure_cron::<T>(CrontabTimer::every_minute(), id, None, queue)
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
        strategy_task_reconcile::StrategyTaskReconcile,
        trigger_evaluation::TriggerEvaluation,
    };

    use super::{JQUANTS_QUEUE, build_crontabs, configure_cron, every_minute_cron};
    use core_application::strategy_task::STRATEGY_TASK_RECONCILE_QUEUE_NAME;

    fn expected_cron<T: TaskHandler>(
        timer: Option<CrontabTimer>,
        id: &str,
        fill: Option<CrontabFill>,
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
                Some(CrontabFill::days(3)),
                None,
            ),
            expected_cron::<ShortRatioIngest>(
                CrontabTimer::daily_at(12, 0).ok(),
                "short_ratio_ingest",
                Some(CrontabFill::days(3)),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<ShortSaleReportIngest>(
                CrontabTimer::daily_at(12, 15).ok(),
                "short_sale_report_ingest",
                Some(CrontabFill::days(3)),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<MarginIngest>(
                CrontabTimer::daily_at(12, 30).ok(),
                "margin_ingest",
                Some(CrontabFill::days(3)),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<PredictionGrading>(
                CrontabTimer::weekly_on(Weekday::Sun, 14, 0).ok(),
                "prediction_grading",
                Some(CrontabFill::weeks(2)),
                None,
            ),
            Some(every_minute_cron::<StrategyTaskReconcile>(
                "strategy_task_reconcile",
                Some(STRATEGY_TASK_RECONCILE_QUEUE_NAME),
            )),
            Some(every_minute_cron::<TriggerEvaluation>(
                "trigger_evaluation",
                None,
            )),
        ]
        .into_iter()
        .collect::<Option<Vec<_>>>();

        assert_eq!(build_crontabs(true, true, true).ok(), expected);
    }

    #[test]
    fn configures_missed_tick_fill_retry_limit_and_jquants_queue() {
        let actual = build_crontabs(true, true, true).ok().map(|crontabs| {
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
                (
                    Some("strategy_task_reconcile".to_string()),
                    None,
                    Some(3),
                    Some("strategy_task_reconcile".to_string()),
                ),
                (Some("trigger_evaluation".to_string()), None, Some(3), None,),
            ]),
        );
    }

    #[rstest]
    #[case::neither_source(false, false, true, vec!["prediction_grading", "strategy_task_reconcile", "trigger_evaluation"])]
    #[case::fred_only(true, false, true, vec!["fred_ingest", "prediction_grading", "strategy_task_reconcile", "trigger_evaluation"])]
    #[case::jquants_only(false, true, true, vec!["short_ratio_ingest", "short_sale_report_ingest", "margin_ingest", "prediction_grading", "strategy_task_reconcile", "trigger_evaluation"])]
    #[case::agent_client_disabled(false, false, false, vec!["prediction_grading", "trigger_evaluation"])]
    fn schedules_only_configured_sources(
        #[case] include_fred: bool,
        #[case] include_jquants: bool,
        #[case] include_strategy_task_reconcile: bool,
        #[case] expected_ids: Vec<&str>,
    ) {
        let actual_ids = build_crontabs(
            include_fred,
            include_jquants,
            include_strategy_task_reconcile,
        )
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
