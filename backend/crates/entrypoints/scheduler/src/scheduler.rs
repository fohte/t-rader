use std::{future::Future, time::Duration};

use chrono::Weekday;
use graphile_worker::{
    Cron, Crontab, CrontabFill, CrontabTimer, CrontabTimerError, TaskHandler, Worker, WorkerOptions,
};
use sqlx::PgPool;

use crate::{
    jobs::{
        DAILY_TIMEOUT, WEEKLY_TIMEOUT,
        daily_bars::DailyBarsIngest,
        earnings_schedule::EarningsScheduleIngest,
        edinet_holdings::ShareholdingStructureIngest,
        equity_master::EquityMasterIngest,
        financial_summary::FinancialSummaryIngest,
        fred::FredIngest,
        ingest_run_recovery::IngestRunRecovery,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        news::NewsAggregation,
        prediction::PredictionGrading,
        valuation::ValuationIngest,
    },
    state::{SchedulerDependencies, SchedulerState},
};

const GRAPHILE_WORKER_SCHEMA: &str = "graphile_worker";
const JQUANTS_QUEUE: &str = "jquants";
const MAX_ATTEMPTS: u16 = 3;
const INGEST_RUN_RECOVERY_INTERVAL_MINUTES: u32 = 5;
pub(crate) const RECOVERABLE_INGEST_JOBS: [(&str, Duration); 12] = [
    (FredIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShortRatioIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShortSaleReportIngest::IDENTIFIER, DAILY_TIMEOUT),
    (MarginIngest::IDENTIFIER, DAILY_TIMEOUT),
    (PredictionGrading::IDENTIFIER, WEEKLY_TIMEOUT),
    (DailyBarsIngest::IDENTIFIER, DAILY_TIMEOUT),
    (EarningsScheduleIngest::IDENTIFIER, DAILY_TIMEOUT),
    (FinancialSummaryIngest::IDENTIFIER, DAILY_TIMEOUT),
    (NewsAggregation::IDENTIFIER, DAILY_TIMEOUT),
    (ValuationIngest::IDENTIFIER, DAILY_TIMEOUT),
    (EquityMasterIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShareholdingStructureIngest::IDENTIFIER, DAILY_TIMEOUT),
];

#[derive(Clone, Copy, Default)]
struct ConfiguredJobs {
    daily_bars: bool,
    fred: bool,
    jquants: bool,
    earnings_schedule: bool,
    financial_summary: bool,
    valuation: bool,
    equity_master: bool,
    shareholding_structure: bool,
}

pub struct Scheduler {
    worker: Worker,
}

impl Scheduler {
    pub async fn initialize(
        pool: PgPool,
        dependencies: SchedulerDependencies,
        shutdown_signal: impl Future<Output = ()> + Send + 'static,
    ) -> Result<Self, String> {
        let crontabs = build_crontabs(ConfiguredJobs {
            daily_bars: dependencies.market_daily_bar_source.is_some(),
            fred: dependencies.fred_source.is_some(),
            jquants: dependencies.short_selling_source.is_some()
                && dependencies.margin_source.is_some(),
            earnings_schedule: dependencies.earnings_schedule_source.is_some(),
            financial_summary: dependencies.financial_summary_source.is_some(),
            valuation: dependencies.valuation_source.is_some(),
            equity_master: dependencies.equity_master_source.is_some(),
            shareholding_structure: dependencies.shareholding_structure_source.is_some(),
        })
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
            .define_job::<DailyBarsIngest>()
            .define_job::<NewsAggregation>()
            .define_job::<EarningsScheduleIngest>()
            .define_job::<FinancialSummaryIngest>()
            .define_job::<ValuationIngest>()
            .define_job::<EquityMasterIngest>()
            .define_job::<ShareholdingStructureIngest>()
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

fn build_crontabs(configured: ConfiguredJobs) -> Result<Vec<Crontab>, CrontabTimerError> {
    let mut crontabs = vec![hourly_cron::<NewsAggregation>(
        "news_aggregation",
        0,
        CrontabFill::hours(3),
        None,
    )?];
    if configured.fred {
        crontabs.push(daily_cron::<FredIngest>("fred_ingest", 11, 30, None)?);
    }
    if configured.jquants {
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
    if configured.earnings_schedule {
        crontabs.push(daily_cron::<EarningsScheduleIngest>(
            "earnings_schedule_ingest",
            12,
            45,
            Some(JQUANTS_QUEUE),
        )?);
    }
    if configured.financial_summary {
        crontabs.push(daily_cron::<FinancialSummaryIngest>(
            "financial_summary_ingest",
            13,
            0,
            Some(JQUANTS_QUEUE),
        )?);
    }
    if configured.equity_master {
        crontabs.push(daily_cron::<EquityMasterIngest>(
            "equity_master_ingest",
            13,
            15,
            Some(JQUANTS_QUEUE),
        )?);
    }
    if configured.shareholding_structure {
        crontabs.push(daily_cron::<ShareholdingStructureIngest>(
            "shareholding_structure_ingest",
            13,
            30,
            Some(JQUANTS_QUEUE),
        )?);
    }
    if configured.valuation {
        crontabs.push(hourly_cron::<ValuationIngest>(
            "valuation_ingest",
            30,
            CrontabFill::hours(3),
            Some(JQUANTS_QUEUE),
        )?);
    }
    if configured.daily_bars {
        crontabs.push(hourly_cron::<DailyBarsIngest>(
            "daily_bars_ingest",
            45,
            CrontabFill::hours(3),
            Some(JQUANTS_QUEUE),
        )?);
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

fn hourly_cron<T: TaskHandler>(
    id: &str,
    utc_minute: u32,
    fill: CrontabFill,
    queue: Option<&str>,
) -> Result<Crontab, CrontabTimerError> {
    Ok(configure_cron::<T>(
        CrontabTimer::hourly_at(utc_minute)?,
        id,
        fill,
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
    use rstest::{fixture, rstest};

    use crate::jobs::{
        daily_bars::DailyBarsIngest,
        earnings_schedule::EarningsScheduleIngest,
        edinet_holdings::ShareholdingStructureIngest,
        equity_master::EquityMasterIngest,
        financial_summary::FinancialSummaryIngest,
        fred::FredIngest,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        news::NewsAggregation,
        prediction::PredictionGrading,
        valuation::ValuationIngest,
    };

    use super::{ConfiguredJobs, JQUANTS_QUEUE, build_crontabs, configure_cron, hourly_cron};

    #[fixture]
    fn all_configured_jobs() -> ConfiguredJobs {
        ConfiguredJobs {
            daily_bars: true,
            fred: true,
            jquants: true,
            earnings_schedule: true,
            financial_summary: true,
            valuation: true,
            equity_master: true,
            shareholding_structure: true,
        }
    }

    fn expected_cron<T: TaskHandler>(
        timer: Option<CrontabTimer>,
        id: &str,
        fill: CrontabFill,
        queue: Option<&str>,
    ) -> Option<Crontab> {
        Some(configure_cron::<T>(timer?, id, fill, queue))
    }

    #[rstest]
    fn schedules_fixed_utc_times_and_serializes_jquants_jobs(all_configured_jobs: ConfiguredJobs) {
        let expected = [
            expected_cron::<NewsAggregation>(
                CrontabTimer::hourly_at(0).ok(),
                "news_aggregation",
                CrontabFill::hours(3),
                None,
            ),
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
            expected_cron::<EarningsScheduleIngest>(
                CrontabTimer::daily_at(12, 45).ok(),
                "earnings_schedule_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<FinancialSummaryIngest>(
                CrontabTimer::daily_at(13, 0).ok(),
                "financial_summary_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<EquityMasterIngest>(
                CrontabTimer::daily_at(13, 15).ok(),
                "equity_master_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<ShareholdingStructureIngest>(
                CrontabTimer::daily_at(13, 30).ok(),
                "shareholding_structure_ingest",
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<ValuationIngest>(
                CrontabTimer::hourly_at(30).ok(),
                "valuation_ingest",
                CrontabFill::hours(3),
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<DailyBarsIngest>(
                CrontabTimer::hourly_at(45).ok(),
                "daily_bars_ingest",
                CrontabFill::hours(3),
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

        assert_eq!(build_crontabs(all_configured_jobs).ok(), expected);
    }

    #[rstest]
    fn configures_missed_tick_fill_retry_limit_and_jquants_queue(
        all_configured_jobs: ConfiguredJobs,
    ) {
        let actual = build_crontabs(all_configured_jobs).ok().map(|crontabs| {
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
                    Some("news_aggregation".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(3),
                    None,
                ),
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
                    Some("earnings_schedule_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("financial_summary_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("equity_master_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("shareholding_structure_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("valuation_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(3),
                    Some("jquants".to_string()),
                ),
                (
                    Some("daily_bars_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
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
    #[case::no_optional_source(ConfiguredJobs::default(), vec!["news_aggregation", "prediction_grading"])]
    #[case::daily_bars_only(ConfiguredJobs { daily_bars: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "daily_bars_ingest", "prediction_grading"])]
    #[case::fred_only(ConfiguredJobs { fred: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "fred_ingest", "prediction_grading"])]
    #[case::existing_jquants_only(ConfiguredJobs { jquants: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "short_ratio_ingest", "short_sale_report_ingest", "margin_ingest", "prediction_grading"])]
    #[case::earnings_schedule_only(ConfiguredJobs { earnings_schedule: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "earnings_schedule_ingest", "prediction_grading"])]
    #[case::financial_summary_only(ConfiguredJobs { financial_summary: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "financial_summary_ingest", "prediction_grading"])]
    #[case::valuation_only(ConfiguredJobs { valuation: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "valuation_ingest", "prediction_grading"])]
    #[case::equity_master_only(ConfiguredJobs { equity_master: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "equity_master_ingest", "prediction_grading"])]
    #[case::shareholding_structure_only(ConfiguredJobs { shareholding_structure: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "shareholding_structure_ingest", "prediction_grading"])]
    fn schedules_only_configured_sources(
        #[case] configured: ConfiguredJobs,
        #[case] expected_ids: Vec<&str>,
    ) {
        let actual_ids = build_crontabs(configured).ok().map(|crontabs| {
            crontabs
                .into_iter()
                .filter_map(|crontab| crontab.options.id)
                .collect::<Vec<_>>()
        });
        let expected_ids = Some(expected_ids.into_iter().map(str::to_string).collect());

        assert_eq!(actual_ids, expected_ids);
    }

    #[test]
    fn hourly_cron_configures_the_requested_minute_and_fill() {
        let expected = expected_cron::<NewsAggregation>(
            CrontabTimer::hourly_at(15).ok(),
            "news_aggregation",
            CrontabFill::hours(1),
            None,
        );

        assert_eq!(
            hourly_cron::<NewsAggregation>("news_aggregation", 15, CrontabFill::hours(1), None,)
                .ok(),
            expected,
        );
    }
}
