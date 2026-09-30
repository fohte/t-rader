mod jobs;
mod state;

use std::sync::Arc;

use chrono::Weekday;
use core_application::indicator_observation_source::SharedIndicatorObservationSource;
use gateway_jquants::JQuantsClient;
use graphile_worker::{
    Crontab, CrontabFill, CrontabTimer, CrontabTimerError, TaskHandler, Worker, WorkerOptions,
};
use sea_orm::DatabaseConnection;

use crate::services::use_cases::UseCases;
use jobs::fred::FredIngest;
use jobs::jquants::{
    EarningsDateIngest, EdinetHoldingsIngest, FinSummaryIngest, MarginIngest, ShortRatioIngest,
    ShortSaleReportIngest, StockMasterSync, ValuationIngest,
};
use jobs::prediction::PredictionGrading;
use state::SchedulerState;

const GRAPHILE_WORKER_SCHEMA: &str = "graphile_worker";
const JQUANTS_QUEUE: &str = "jquants";
const MAX_ATTEMPTS: u16 = 3;

pub async fn initialize(
    db: DatabaseConnection,
    use_cases: UseCases,
    fred_source: Option<SharedIndicatorObservationSource>,
    jquants_client: Option<Arc<JQuantsClient>>,
) -> Result<Worker, String> {
    let crontabs = build_crontabs(fred_source.is_some(), jquants_client.is_some())
        .map_err(|error| error.to_string())?;
    let pool = db.get_postgres_connection_pool().clone();
    let state = SchedulerState {
        db,
        use_cases,
        fred_source,
        jquants_client,
    };
    let options = WorkerOptions::default()
        .pg_pool(pool)
        .schema(GRAPHILE_WORKER_SCHEMA)
        .concurrency(2)
        .add_extension(state)
        .define_job::<FredIngest>()
        .define_job::<StockMasterSync>()
        .define_job::<ShortSaleReportIngest>()
        .define_job::<ShortRatioIngest>()
        .define_job::<MarginIngest>()
        .define_job::<FinSummaryIngest>()
        .define_job::<EarningsDateIngest>()
        .define_job::<EdinetHoldingsIngest>()
        .define_job::<ValuationIngest>()
        .define_job::<PredictionGrading>()
        .with_crons(crontabs);
    options.init().await.map_err(|error| error.to_string())
}

fn build_crontabs(
    include_fred: bool,
    include_jquants: bool,
) -> Result<Vec<Crontab>, CrontabTimerError> {
    let mut crontabs = Vec::new();
    if include_fred {
        crontabs.push(daily_cron::<FredIngest>(
            "fred_ingest",
            11,
            30,
            CrontabFill::days(3),
            None,
        )?);
    }
    if include_jquants {
        crontabs.extend([
            daily_cron::<StockMasterSync>(
                "stock_master_sync",
                12,
                0,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<ShortSaleReportIngest>(
                "short_sale_report_ingest",
                12,
                15,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<ShortRatioIngest>(
                "short_ratio_ingest",
                12,
                30,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<MarginIngest>(
                "margin_ingest",
                12,
                45,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<FinSummaryIngest>(
                "fin_summary_ingest",
                13,
                0,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<EarningsDateIngest>(
                "earnings_date_ingest",
                13,
                15,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            daily_cron::<EdinetHoldingsIngest>(
                "edinet_holdings_ingest",
                13,
                30,
                CrontabFill::days(3),
                Some(JQUANTS_QUEUE),
            )?,
            hourly_cron::<ValuationIngest>(
                "valuation_ingest",
                5,
                CrontabFill::hours(2),
                Some(JQUANTS_QUEUE),
            )?,
        ]);
    }
    crontabs.push(weekly_cron::<PredictionGrading>(
        "prediction_grading",
        Weekday::Sun,
        14,
        0,
        CrontabFill::weeks(2),
        None,
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
    fill: CrontabFill,
    queue: Option<&str>,
) -> Result<Crontab, CrontabTimerError> {
    Ok(configure_cron::<T>(
        CrontabTimer::daily_at(utc_hour, utc_minute)?,
        id,
        fill,
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
    fill: CrontabFill,
    queue: Option<&str>,
) -> Result<Crontab, CrontabTimerError> {
    Ok(configure_cron::<T>(
        CrontabTimer::weekly_on(weekday, utc_hour, utc_minute)?,
        id,
        fill,
        queue,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::Weekday;
    use graphile_worker::{Crontab, CrontabFill, CrontabTimer};

    use super::{JQUANTS_QUEUE, MAX_ATTEMPTS, build_crontabs};

    fn expected(
        identifier: &str,
        timer: CrontabTimer,
        fill: CrontabFill,
        queue: Option<&str>,
    ) -> Crontab {
        let mut crontab = Crontab::new(timer, identifier);
        crontab.options.id = Some(identifier.to_string());
        crontab.options.fill = Some(fill);
        crontab.options.max = Some(MAX_ATTEMPTS);
        crontab.options.queue = queue.map(str::to_string);
        crontab
    }

    #[test]
    fn schedules_jobs_at_fixed_jst_night_times_and_serializes_jquants() {
        assert_eq!(
            build_crontabs(true, true).expect("valid cron definitions"),
            vec![
                expected(
                    "fred_ingest",
                    CrontabTimer::daily_at(11, 30).expect("valid daily timer"),
                    CrontabFill::days(3),
                    None,
                ),
                expected(
                    "stock_master_sync",
                    CrontabTimer::daily_at(12, 0).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "short_sale_report_ingest",
                    CrontabTimer::daily_at(12, 15).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "short_ratio_ingest",
                    CrontabTimer::daily_at(12, 30).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "margin_ingest",
                    CrontabTimer::daily_at(12, 45).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "fin_summary_ingest",
                    CrontabTimer::daily_at(13, 0).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "earnings_date_ingest",
                    CrontabTimer::daily_at(13, 15).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "edinet_holdings_ingest",
                    CrontabTimer::daily_at(13, 30).expect("valid daily timer"),
                    CrontabFill::days(3),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "valuation_ingest",
                    CrontabTimer::hourly_at(5).expect("valid hourly timer"),
                    CrontabFill::hours(2),
                    Some(JQUANTS_QUEUE),
                ),
                expected(
                    "prediction_grading",
                    CrontabTimer::weekly_on(Weekday::Sun, 14, 0).expect("valid weekly timer"),
                    CrontabFill::weeks(2),
                    None,
                ),
            ],
        );
    }

    #[test]
    fn omits_schedules_for_unconfigured_external_sources() {
        assert_eq!(
            build_crontabs(false, false).expect("valid cron definitions"),
            vec![expected(
                "prediction_grading",
                CrontabTimer::weekly_on(Weekday::Sun, 14, 0).expect("valid weekly timer"),
                CrontabFill::weeks(2),
                None,
            )],
        );
    }
}
