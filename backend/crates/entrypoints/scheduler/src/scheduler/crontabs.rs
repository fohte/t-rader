use chrono::Weekday;
use core_application::{
    event_dispatch_job::EVENT_DISPATCH_QUEUE_NAME,
    ingest_status::{FED_CALENDAR_EVENT_INGEST_JOB, FRED_RELEASE_DATES_INGEST_JOB},
    strategy_task::STRATEGY_TASK_RECONCILE_QUEUE_NAME,
};
use graphile_worker::{
    Crontab, CrontabFill, CrontabTimer, CrontabTimerError, CrontabValue, TaskHandler,
};

use crate::jobs::{
    alpha_vantage::AlphaVantageCalendarIngest,
    boj_calendar::BojCalendarEventIngest,
    daily_bars::DailyBarsIngest,
    e_stat_calendar::EStatCalendarIngest,
    earnings_schedule::EarningsScheduleIngest,
    ecb_calendar::EcbCalendarEventIngest,
    edinet_holdings::ShareholdingStructureIngest,
    eoddata_kospi::EodDataKospiIngest,
    equity_master::EquityMasterIngest,
    event_dispatch::EventDispatch,
    fed_calendar::FedCalendarEventIngest,
    financial_summary::FinancialSummaryIngest,
    fred::FredIngest,
    fred_release_dates::FredReleaseDatesIngest,
    jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
    news::NewsAggregation,
    news_content::NewsContentFetch,
    paper_order_filling::PaperOrderFilling,
    prediction::PredictionGrading,
    strategy_task_reconcile::StrategyTaskReconcile,
    trigger_evaluation::TriggerEvaluation,
    twse::TwseIndexIngest,
    us_stock_bars::{
        UsStockBarsIngest, UsStockHourlyBarsIngest, UsStockMinuteBarsDownsample,
        UsStockMinuteBarsIngest,
    },
    us_stock_master::UsStockMasterIngest,
    valuation::ValuationIngest,
};

const JQUANTS_QUEUE: &str = "jquants";
const ALPACA_QUEUE: &str = "alpaca";
const ALPACA_MINUTE_QUEUE: &str = "alpaca-minute";
const ALPACA_HOURLY_QUEUE: &str = "alpaca-hourly";
const FRED_QUEUE: &str = "fred";
const EODDATA_QUEUE: &str = "eoddata";
const MAX_ATTEMPTS: u16 = 3;
const BUSINESS_DAY_INGEST_MAX_ATTEMPTS: u16 = 10;

#[derive(Clone, Copy, Default)]
pub(super) struct ConfiguredJobs {
    pub(super) daily_bars: bool,
    pub(super) fred: bool,
    pub(super) eoddata_kospi: bool,
    pub(super) twse: bool,
    pub(super) e_stat_calendar: bool,
    pub(super) alpha_vantage_calendar: bool,
    pub(super) fred_release_dates: bool,
    pub(super) boj_calendar: bool,
    pub(super) ecb_calendar: bool,
    pub(super) fed_calendar: bool,
    pub(super) jquants: bool,
    pub(super) earnings_schedule: bool,
    pub(super) financial_summary: bool,
    pub(super) valuation: bool,
    pub(super) equity_master: bool,
    pub(super) us_stock_master: bool,
    pub(super) us_stock_bars: bool,
    pub(super) shareholding_structure: bool,
    pub(super) strategy_task_reconcile: bool,
    pub(super) news_content: bool,
}
pub(super) fn build_crontabs(
    configured: ConfiguredJobs,
) -> Result<Vec<Crontab>, CrontabTimerError> {
    let mut crontabs = vec![hourly_cron::<NewsAggregation>(
        "news_aggregation",
        0,
        CrontabFill::hours(3),
        None,
    )?];
    if configured.fred {
        crontabs.push(daily_cron::<FredIngest>(
            "fred_ingest",
            11,
            30,
            Some(FRED_QUEUE),
        )?);
    }
    if configured.eoddata_kospi {
        crontabs.push(daily_cron::<EodDataKospiIngest>(
            "eoddata_kospi_ingest",
            9,
            0,
            Some(EODDATA_QUEUE),
        )?);
    }
    if configured.twse {
        // 台湾時間 15:00 (UTC 07:00) に実行し、取引終了後の公開を待つ。
        crontabs.push(daily_cron::<TwseIndexIngest>(
            "twse_index_ingest",
            7,
            0,
            Some(TWSE_QUEUE),
        )?);
    }
    if configured.fred_release_dates {
        crontabs.push(daily_cron::<FredReleaseDatesIngest>(
            FRED_RELEASE_DATES_INGEST_JOB,
            11,
            45,
            Some(FRED_QUEUE),
        )?);
    }
    if configured.e_stat_calendar {
        crontabs.push(daily_cron::<EStatCalendarIngest>(
            "e_stat_calendar_ingest",
            11,
            45,
            None,
        )?);
    }
    if configured.alpha_vantage_calendar {
        crontabs.push(daily_cron::<AlphaVantageCalendarIngest>(
            "alpha_vantage_calendar_ingest",
            11,
            45,
            None,
        )?);
    }
    if configured.boj_calendar {
        crontabs.push(daily_cron::<BojCalendarEventIngest>(
            "boj_calendar_event_ingest",
            11,
            45,
            None,
        )?);
    }
    if configured.ecb_calendar {
        crontabs.push(daily_cron::<EcbCalendarEventIngest>(
            "ecb_calendar_event_ingest",
            12,
            0,
            None,
        )?);
    }
    if configured.fed_calendar {
        crontabs.push(daily_cron::<FedCalendarEventIngest>(
            FED_CALENDAR_EVENT_INGEST_JOB,
            12,
            15,
            None,
        )?);
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
    if configured.us_stock_master {
        crontabs.push(daily_cron::<UsStockMasterIngest>(
            "us_stock_master_ingest",
            13,
            45,
            None,
        )?);
    }
    if configured.us_stock_bars {
        crontabs.push(weekday_daily_cron::<UsStockBarsIngest>(
            "us_stock_bars_ingest",
            22,
            20,
            Some(ALPACA_QUEUE),
        )?);
        crontabs.push(hourly_cron::<UsStockMinuteBarsIngest>(
            "us_stock_minute_bars_ingest",
            20,
            CrontabFill::hours(3),
            Some(ALPACA_MINUTE_QUEUE),
        )?);
        // 新しく日足対象になった銘柄のバックフィルを始めるため、ジョブ自体は定期実行する。
        crontabs.push(hourly_cron::<UsStockHourlyBarsIngest>(
            "us_stock_hourly_bars_ingest",
            40,
            CrontabFill::hours(3),
            Some(ALPACA_HOURLY_QUEUE),
        )?);
        crontabs.push(daily_cron::<UsStockMinuteBarsDownsample>(
            "us_stock_minute_bars_downsample",
            0,
            5,
            Some(ALPACA_MINUTE_QUEUE),
        )?);
    }
    if configured.valuation {
        crontabs.push(weekday_daily_cron::<ValuationIngest>(
            "valuation_ingest",
            7,
            30,
            Some(JQUANTS_QUEUE),
        )?);
    }
    if configured.daily_bars {
        crontabs.push(weekday_daily_cron::<DailyBarsIngest>(
            "daily_bars_ingest",
            7,
            45,
            Some(JQUANTS_QUEUE),
        )?);
    }
    crontabs.push(weekly_cron::<PredictionGrading>(
        "prediction_grading",
        Weekday::Sun,
        14,
        0,
    )?);
    crontabs.push(daily_cron::<PaperOrderFilling>(
        core_application::ingest_status::PAPER_ORDER_FILLING_JOB,
        16,
        0,
        None,
    )?);
    if configured.strategy_task_reconcile {
        crontabs.push(every_minute_cron::<StrategyTaskReconcile>(
            "strategy_task_reconcile",
            Some(STRATEGY_TASK_RECONCILE_QUEUE_NAME),
        ));
    }
    if configured.news_content {
        crontabs.push(every_n_minutes_cron::<NewsContentFetch>(
            "news_content_fetch",
            10,
            Some(NEWS_CONTENT_QUEUE),
        )?);
    }
    crontabs.push(every_minute_cron::<EventDispatch>(
        "event_dispatch",
        Some(EVENT_DISPATCH_QUEUE_NAME),
    ));
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

fn weekday_daily_cron<T: TaskHandler>(
    id: &str,
    utc_hour: u32,
    utc_minute: u32,
    queue: Option<&str>,
) -> Result<Crontab, CrontabTimerError> {
    let mut timer = CrontabTimer::daily_at(utc_hour, utc_minute)?;
    timer.dows = vec![CrontabValue::Range(1, 5)];
    let mut crontab = configure_cron::<T>(timer, id, Some(CrontabFill::hours(3)), queue);
    crontab.options.max = Some(BUSINESS_DAY_INGEST_MAX_ATTEMPTS);
    Ok(crontab)
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
        Some(fill),
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

fn every_n_minutes_cron<T: TaskHandler>(
    id: &str,
    minutes: u32,
    queue: Option<&str>,
) -> Result<Crontab, CrontabTimerError> {
    Ok(configure_cron::<T>(
        CrontabTimer::every_n_minutes(minutes)?,
        id,
        None,
        queue,
    ))
}

const NEWS_CONTENT_QUEUE: &str = "news_content";
const TWSE_QUEUE: &str = "twse";

#[cfg(test)]
mod tests {
    use chrono::Weekday;
    use graphile_worker::{Crontab, CrontabFill, CrontabTimer, CrontabValue, TaskHandler};
    use rstest::{fixture, rstest};

    use crate::jobs::{
        alpha_vantage::AlphaVantageCalendarIngest,
        boj_calendar::BojCalendarEventIngest,
        daily_bars::DailyBarsIngest,
        e_stat_calendar::EStatCalendarIngest,
        earnings_schedule::EarningsScheduleIngest,
        ecb_calendar::EcbCalendarEventIngest,
        edinet_holdings::ShareholdingStructureIngest,
        eoddata_kospi::EodDataKospiIngest,
        equity_master::EquityMasterIngest,
        event_dispatch::EventDispatch,
        fed_calendar::FedCalendarEventIngest,
        financial_summary::FinancialSummaryIngest,
        fred::FredIngest,
        fred_release_dates::FredReleaseDatesIngest,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        news::NewsAggregation,
        news_content::NewsContentFetch,
        paper_order_filling::PaperOrderFilling,
        prediction::PredictionGrading,
        strategy_task_reconcile::StrategyTaskReconcile,
        trigger_evaluation::TriggerEvaluation,
        twse::TwseIndexIngest,
        us_stock_bars::{
            UsStockBarsIngest, UsStockHourlyBarsIngest, UsStockMinuteBarsDownsample,
            UsStockMinuteBarsIngest,
        },
        us_stock_master::UsStockMasterIngest,
        valuation::ValuationIngest,
    };

    use super::{
        ALPACA_HOURLY_QUEUE, ALPACA_MINUTE_QUEUE, ALPACA_QUEUE, ConfiguredJobs, EODDATA_QUEUE,
        EVENT_DISPATCH_QUEUE_NAME, FED_CALENDAR_EVENT_INGEST_JOB, FRED_QUEUE, JQUANTS_QUEUE,
        NEWS_CONTENT_QUEUE, STRATEGY_TASK_RECONCILE_QUEUE_NAME, TWSE_QUEUE, build_crontabs,
        configure_cron, every_minute_cron, hourly_cron,
    };

    #[fixture]
    fn all_configured_jobs() -> ConfiguredJobs {
        ConfiguredJobs {
            daily_bars: true,
            fred: true,
            eoddata_kospi: true,
            twse: true,
            e_stat_calendar: true,
            alpha_vantage_calendar: true,
            fred_release_dates: true,
            boj_calendar: true,
            ecb_calendar: true,
            fed_calendar: true,
            jquants: true,
            earnings_schedule: true,
            financial_summary: true,
            valuation: true,
            equity_master: true,
            us_stock_master: true,
            us_stock_bars: true,
            shareholding_structure: true,
            strategy_task_reconcile: true,
            news_content: true,
        }
    }

    fn expected_cron<T: TaskHandler>(
        timer: Option<CrontabTimer>,
        id: &str,
        fill: CrontabFill,
        queue: Option<&str>,
    ) -> Option<Crontab> {
        Some(configure_cron::<T>(timer?, id, Some(fill), queue))
    }

    fn expected_weekday_cron<T: TaskHandler>(
        hour: u32,
        minute: u32,
        id: &str,
        queue: Option<&str>,
    ) -> Option<Crontab> {
        let mut crontab = expected_cron::<T>(
            Some(CrontabTimer {
                minutes: vec![CrontabValue::Number(minute)],
                hours: vec![CrontabValue::Number(hour)],
                dows: vec![CrontabValue::Range(1, 5)],
                ..Default::default()
            }),
            id,
            CrontabFill::hours(3),
            queue,
        )?;
        crontab.options.max = Some(10);
        Some(crontab)
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
                Some(FRED_QUEUE),
            ),
            expected_cron::<EodDataKospiIngest>(
                CrontabTimer::daily_at(9, 0).ok(),
                "eoddata_kospi_ingest",
                CrontabFill::days(3),
                Some(EODDATA_QUEUE),
            ),
            expected_cron::<TwseIndexIngest>(
                CrontabTimer::daily_at(7, 0).ok(),
                "twse_index_ingest",
                CrontabFill::days(3),
                Some(TWSE_QUEUE),
            ),
            expected_cron::<FredReleaseDatesIngest>(
                CrontabTimer::daily_at(11, 45).ok(),
                "fred_release_dates_ingest",
                CrontabFill::days(3),
                Some(FRED_QUEUE),
            ),
            expected_cron::<EStatCalendarIngest>(
                CrontabTimer::daily_at(11, 45).ok(),
                "e_stat_calendar_ingest",
                CrontabFill::days(3),
                None,
            ),
            expected_cron::<AlphaVantageCalendarIngest>(
                CrontabTimer::daily_at(11, 45).ok(),
                "alpha_vantage_calendar_ingest",
                CrontabFill::days(3),
                None,
            ),
            expected_cron::<BojCalendarEventIngest>(
                CrontabTimer::daily_at(11, 45).ok(),
                "boj_calendar_event_ingest",
                CrontabFill::days(3),
                None,
            ),
            expected_cron::<EcbCalendarEventIngest>(
                CrontabTimer::daily_at(12, 0).ok(),
                "ecb_calendar_event_ingest",
                CrontabFill::days(3),
                None,
            ),
            expected_cron::<FedCalendarEventIngest>(
                CrontabTimer::daily_at(12, 15).ok(),
                FED_CALENDAR_EVENT_INGEST_JOB,
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
            expected_cron::<UsStockMasterIngest>(
                CrontabTimer::daily_at(13, 45).ok(),
                "us_stock_master_ingest",
                CrontabFill::days(3),
                None,
            ),
            expected_weekday_cron::<UsStockBarsIngest>(
                22,
                20,
                "us_stock_bars_ingest",
                Some(ALPACA_QUEUE),
            ),
            expected_cron::<UsStockMinuteBarsIngest>(
                CrontabTimer::hourly_at(20).ok(),
                "us_stock_minute_bars_ingest",
                CrontabFill::hours(3),
                Some(ALPACA_MINUTE_QUEUE),
            ),
            expected_cron::<UsStockHourlyBarsIngest>(
                CrontabTimer::hourly_at(40).ok(),
                "us_stock_hourly_bars_ingest",
                CrontabFill::hours(3),
                Some(ALPACA_HOURLY_QUEUE),
            ),
            expected_cron::<UsStockMinuteBarsDownsample>(
                CrontabTimer::daily_at(0, 5).ok(),
                "us_stock_minute_bars_downsample",
                CrontabFill::days(3),
                Some(ALPACA_MINUTE_QUEUE),
            ),
            expected_weekday_cron::<ValuationIngest>(
                7,
                30,
                "valuation_ingest",
                Some(JQUANTS_QUEUE),
            ),
            expected_weekday_cron::<DailyBarsIngest>(
                7,
                45,
                "daily_bars_ingest",
                Some(JQUANTS_QUEUE),
            ),
            expected_cron::<PredictionGrading>(
                CrontabTimer::weekly_on(Weekday::Sun, 14, 0).ok(),
                "prediction_grading",
                CrontabFill::weeks(2),
                None,
            ),
            expected_cron::<PaperOrderFilling>(
                CrontabTimer::daily_at(16, 0).ok(),
                core_application::ingest_status::PAPER_ORDER_FILLING_JOB,
                CrontabFill::days(3),
                None,
            ),
            Some(every_minute_cron::<StrategyTaskReconcile>(
                "strategy_task_reconcile",
                Some(STRATEGY_TASK_RECONCILE_QUEUE_NAME),
            )),
            CrontabTimer::every_n_minutes(10).ok().map(|timer| {
                configure_cron::<NewsContentFetch>(
                    timer,
                    "news_content_fetch",
                    None,
                    Some(NEWS_CONTENT_QUEUE),
                )
            }),
            Some(every_minute_cron::<EventDispatch>(
                "event_dispatch",
                Some(EVENT_DISPATCH_QUEUE_NAME),
            )),
            Some(every_minute_cron::<TriggerEvaluation>(
                "trigger_evaluation",
                None,
            )),
        ]
        .into_iter()
        .collect::<Option<Vec<_>>>();

        assert_eq!(build_crontabs(all_configured_jobs).ok(), expected);
    }

    #[rstest]
    fn configures_missed_tick_fill_retry_limit_and_source_queues(
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
                    Some(FRED_QUEUE.to_string()),
                ),
                (
                    Some("eoddata_kospi_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some(EODDATA_QUEUE.to_string()),
                ),
                (
                    Some("twse_index_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some(TWSE_QUEUE.to_string()),
                ),
                (
                    Some("fred_release_dates_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some(FRED_QUEUE.to_string()),
                ),
                (
                    Some("e_stat_calendar_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some("alpha_vantage_calendar_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some("boj_calendar_event_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some("ecb_calendar_event_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some(FED_CALENDAR_EVENT_INGEST_JOB.to_string()),
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
                    Some("us_stock_master_ingest".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some("us_stock_bars_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(10),
                    Some(ALPACA_QUEUE.to_string()),
                ),
                (
                    Some("us_stock_minute_bars_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(3),
                    Some(ALPACA_MINUTE_QUEUE.to_string()),
                ),
                (
                    Some("us_stock_hourly_bars_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(3),
                    Some(ALPACA_HOURLY_QUEUE.to_string()),
                ),
                (
                    Some("us_stock_minute_bars_downsample".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    Some(ALPACA_MINUTE_QUEUE.to_string()),
                ),
                (
                    Some("valuation_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(10),
                    Some("jquants".to_string()),
                ),
                (
                    Some("daily_bars_ingest".to_string()),
                    Some(CrontabFill::hours(3)),
                    Some(10),
                    Some("jquants".to_string()),
                ),
                (
                    Some("prediction_grading".to_string()),
                    Some(CrontabFill::weeks(2)),
                    Some(3),
                    None,
                ),
                (
                    Some("paper_order_filling".to_string()),
                    Some(CrontabFill::days(3)),
                    Some(3),
                    None,
                ),
                (
                    Some("strategy_task_reconcile".to_string()),
                    None,
                    Some(3),
                    Some(STRATEGY_TASK_RECONCILE_QUEUE_NAME.to_string()),
                ),
                (
                    Some("news_content_fetch".to_string()),
                    None,
                    Some(3),
                    Some(NEWS_CONTENT_QUEUE.to_string()),
                ),
                (
                    Some("event_dispatch".to_string()),
                    None,
                    Some(3),
                    Some(EVENT_DISPATCH_QUEUE_NAME.to_string()),
                ),
                (Some("trigger_evaluation".to_string()), None, Some(3), None,),
            ]),
        );
    }

    #[rstest]
    #[case::no_optional_source(ConfiguredJobs::default(), vec!["news_aggregation", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::daily_bars_only(ConfiguredJobs { daily_bars: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "daily_bars_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::fred_only(ConfiguredJobs { fred: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "fred_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::eoddata_kospi_only(ConfiguredJobs { eoddata_kospi: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "eoddata_kospi_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::twse_only(ConfiguredJobs { twse: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "twse_index_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::e_stat_calendar_only(ConfiguredJobs { e_stat_calendar: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "e_stat_calendar_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::boj_calendar_only(ConfiguredJobs { boj_calendar: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "boj_calendar_event_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::ecb_calendar_only(ConfiguredJobs { ecb_calendar: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "ecb_calendar_event_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::fed_calendar_only(ConfiguredJobs { fed_calendar: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "fed_calendar_event_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::alpha_vantage_calendar_only(ConfiguredJobs { alpha_vantage_calendar: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "alpha_vantage_calendar_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::fred_release_dates_only(ConfiguredJobs { fred_release_dates: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "fred_release_dates_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::existing_jquants_only(ConfiguredJobs { jquants: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "short_ratio_ingest", "short_sale_report_ingest", "margin_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::earnings_schedule_only(ConfiguredJobs { earnings_schedule: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "earnings_schedule_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::financial_summary_only(ConfiguredJobs { financial_summary: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "financial_summary_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::valuation_only(ConfiguredJobs { valuation: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "valuation_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::equity_master_only(ConfiguredJobs { equity_master: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "equity_master_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::us_stock_master_only(ConfiguredJobs { us_stock_master: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "us_stock_master_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::us_stock_bars_only(ConfiguredJobs { us_stock_bars: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "us_stock_bars_ingest", "us_stock_minute_bars_ingest", "us_stock_hourly_bars_ingest", "us_stock_minute_bars_downsample", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::shareholding_structure_only(ConfiguredJobs { shareholding_structure: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "shareholding_structure_ingest", "prediction_grading", "paper_order_filling", "event_dispatch", "trigger_evaluation"])]
    #[case::strategy_task_reconcile_enabled(ConfiguredJobs { strategy_task_reconcile: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "prediction_grading", "paper_order_filling", "strategy_task_reconcile", "event_dispatch", "trigger_evaluation"])]
    #[case::news_content_enabled(ConfiguredJobs { news_content: true, ..ConfiguredJobs::default() }, vec!["news_aggregation", "prediction_grading", "paper_order_filling", "news_content_fetch", "event_dispatch", "trigger_evaluation"])]
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
        let expected_ids = expected_ids
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected_ids = Some(expected_ids);

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
