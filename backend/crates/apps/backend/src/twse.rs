use std::sync::Arc;

use chrono::{Months, NaiveDate, Utc};
use chrono_tz::Asia::Taipei;
use core_application::indicator_observation::{
    IndicatorObservationIngestResult, IndicatorObservationIngestSeriesResult, TWSE_SERIES,
};
use core_application::indicator_observation_batch_source::SharedIndicatorObservationBatchSource;
use gateway_twse::TwseClient;

use crate::startup::StartupError;
use backend::cli::RunMode;

pub(super) fn worker_source(
    run_mode: RunMode,
    redis_url: &str,
) -> Result<Option<SharedIndicatorObservationBatchSource>, StartupError> {
    if !run_mode.starts_worker() {
        return Ok(None);
    }

    let client = TwseClient::new(redis_url).map_err(|error| {
        StartupError::Config(format!("failed to initialize TWSE client: {error}"))
    })?;
    tracing::info!("TWSE index source initialized");
    Ok(Some(Arc::new(client)))
}

pub(super) async fn backfill(
    db: sea_orm::DatabaseConnection,
    redis_url: &str,
) -> Result<(), StartupError> {
    let today = Utc::now().with_timezone(&Taipei).date_naive();
    let start = backfill_start(today).ok_or_else(|| {
        StartupError::Config("failed to calculate TWSE backfill start".to_string())
    })?;
    let source: SharedIndicatorObservationBatchSource =
        Arc::new(TwseClient::new(redis_url).map_err(|error| {
            StartupError::Config(format!("failed to initialize TWSE client: {error}"))
        })?);
    let use_cases = backend::services::use_cases::build_use_cases(db);
    let result = use_cases
        .indicator_observations()
        .ingest_batch(source.as_ref(), TWSE_SERIES, Some(start), today)
        .await;
    if let Some(error) = failure_message(&result) {
        return Err(StartupError::Config(error));
    }

    let upserted: usize = result
        .series
        .iter()
        .filter_map(|outcome| match outcome {
            IndicatorObservationIngestSeriesResult::Succeeded { upserted, .. } => Some(*upserted),
            IndicatorObservationIngestSeriesResult::Failed { .. } => None,
        })
        .sum();
    tracing::info!(%start, %today, upserted, "TWSE index backfill completed");
    Ok(())
}

fn backfill_start(today: NaiveDate) -> Option<NaiveDate> {
    today.checked_sub_months(Months::new(24))
}

fn failure_message(result: &IndicatorObservationIngestResult) -> Option<String> {
    let failures = result
        .series
        .iter()
        .filter_map(|outcome| match outcome {
            IndicatorObservationIngestSeriesResult::Succeeded {
                series_id,
                upserted: 0,
            } => Some(format!("{series_id}: no observations were imported")),
            IndicatorObservationIngestSeriesResult::Succeeded { .. } => None,
            IndicatorObservationIngestSeriesResult::Failed { series_id, error } => {
                Some(format!("{series_id}: {error}"))
            }
        })
        .collect::<Vec<_>>();
    (!failures.is_empty()).then(|| format!("TWSE index backfill failed: {}", failures.join("; ")))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::indicator_observation::{
        IndicatorObservationIngestResult, IndicatorObservationIngestSeriesResult,
    };
    use rstest::rstest;

    use super::{backfill_start, failure_message};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn backfill_start_is_24_calendar_months_before_the_requested_end_date() {
        assert_eq!(backfill_start(date(2026, 10, 6)), Some(date(2024, 10, 6)));
    }

    #[rstest]
    #[case::all_succeeded(
        IndicatorObservationIngestResult {
            series: vec![
                IndicatorObservationIngestSeriesResult::Succeeded {
                    series_id: "SERIES_ALPHA".to_string(),
                    upserted: 3,
                },
            ],
        },
        None,
    )]
    #[case::one_failed(
        IndicatorObservationIngestResult {
            series: vec![
                IndicatorObservationIngestSeriesResult::Succeeded {
                    series_id: "SERIES_ALPHA".to_string(),
                    upserted: 3,
                },
                IndicatorObservationIngestSeriesResult::Failed {
                    series_id: "SERIES_BETA".to_string(),
                    error: "synthetic source error".to_string(),
                },
            ],
        },
        Some("TWSE index backfill failed: SERIES_BETA: synthetic source error".to_string()),
    )]
    #[case::no_observations(
        IndicatorObservationIngestResult {
            series: vec![
                IndicatorObservationIngestSeriesResult::Succeeded {
                    series_id: "SERIES_ALPHA".to_string(),
                    upserted: 0,
                },
                IndicatorObservationIngestSeriesResult::Succeeded {
                    series_id: "SERIES_BETA".to_string(),
                    upserted: 0,
                },
            ],
        },
        Some("TWSE index backfill failed: SERIES_ALPHA: no observations were imported; SERIES_BETA: no observations were imported".to_string()),
    )]
    fn failure_message_reports_any_series_failure(
        #[case] result: IndicatorObservationIngestResult,
        #[case] expected: Option<String>,
    ) {
        assert_eq!(failure_message(&result), expected);
    }
}
