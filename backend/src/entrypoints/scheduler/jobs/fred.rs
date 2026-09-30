use core_application::indicator_observation::IndicatorObservationIngestSeriesResult;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct FredIngest;

impl TaskHandler for FredIngest {
    const IDENTIFIER: &'static str = "fred_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = state
                    .fred_source
                    .as_deref()
                    .ok_or_else(|| "FRED source is not configured".to_string())?;
                let result = state
                    .use_cases
                    .indicator_observations()
                    .ingest(source)
                    .await;
                summarize_series(result.series)
            },
        )
        .await
    }
}

fn summarize_series(
    series: impl IntoIterator<Item = IndicatorObservationIngestSeriesResult>,
) -> Result<(), String> {
    let failures = series
        .into_iter()
        .filter_map(|outcome| match outcome {
            IndicatorObservationIngestSeriesResult::Succeeded {
                series_id,
                upserted,
            } => {
                tracing::debug!(%series_id, upserted, "FRED series ingest completed");
                None
            }
            IndicatorObservationIngestSeriesResult::Failed { series_id, error } => {
                tracing::warn!(%series_id, %error, "FRED series ingest failed");
                Some(format!("{series_id}: {error}"))
            }
        })
        .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use core_application::indicator_observation::IndicatorObservationIngestSeriesResult;
    use rstest::rstest;

    use super::summarize_series;

    #[rstest]
    #[case::all_succeeded(
        vec![IndicatorObservationIngestSeriesResult::Succeeded {
            series_id: "sample-series-a".to_string(),
            upserted: 2,
        }],
        Ok(()),
    )]
    #[case::one_failed_after_success(
        vec![
            IndicatorObservationIngestSeriesResult::Succeeded {
                series_id: "sample-series-a".to_string(),
                upserted: 2,
            },
            IndicatorObservationIngestSeriesResult::Failed {
                series_id: "sample-series-b".to_string(),
                error: "sample failure".to_string(),
            },
        ],
        Err("sample-series-b: sample failure".to_string()),
    )]
    #[case::multiple_failures_are_aggregated(
        vec![
            IndicatorObservationIngestSeriesResult::Failed {
                series_id: "sample-series-a".to_string(),
                error: "first failure".to_string(),
            },
            IndicatorObservationIngestSeriesResult::Failed {
                series_id: "sample-series-b".to_string(),
                error: "second failure".to_string(),
            },
        ],
        Err("sample-series-a: first failure; sample-series-b: second failure".to_string()),
    )]
    fn summarizes_ingest_results(
        #[case] series: Vec<IndicatorObservationIngestSeriesResult>,
        #[case] expected: Result<(), String>,
    ) {
        assert_eq!(summarize_series(series), expected);
    }
}
