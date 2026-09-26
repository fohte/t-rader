//! FRED (Federal Reserve Economic Data) からマクロ指標 (ドル円、VIX、米 10 年債利回り、
//! 日経 225) の日次観測値を定期的に取り込む。`indicator` 行はこの取り込み処理が唯一の
//! 書き込み元で、未登録なら初回取り込み時に作る。

use std::time::Duration;

use chrono::NaiveDate;
use core_application::{
    IndicatorObservationSource, IndicatorObservationSourceError, SharedIndicatorObservationSource,
};
use core_domain::IndicatorObservation;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use tokio::task::JoinHandle;

use crate::entities::{indicator, indicator_observation};
use crate::error::AppError;

#[derive(Debug, thiserror::Error)]
enum FredIngestError {
    #[error(transparent)]
    App(#[from] AppError),

    #[error(transparent)]
    Source(#[from] IndicatorObservationSourceError),
}

/// poll task のデフォルト実行間隔。対象系列はいずれも日次更新のため 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 格納済み最新日からさかのぼって再取得する日数。DEXJPUS は FRED への反映が週 1 回まとめてで
/// 最大 1 週間ほど遅れるため、直近サイクルで未反映だった日を拾えるだけの幅を持たせる。
const LOOKBACK_DAYS: i64 = 10;

/// 取り込み対象の 1 系列の定義。FRED の系列 ID と `indicator` 行の対応をここに持つ。
struct SeriesDef {
    fred_series_id: &'static str,
    indicator_id: &'static str,
    indicator_name: &'static str,
    indicator_kind: &'static str,
}

const SERIES: &[SeriesDef] = &[
    SeriesDef {
        fred_series_id: "DEXJPUS",
        indicator_id: "USDJPY",
        indicator_name: "ドル円",
        indicator_kind: "fx",
    },
    SeriesDef {
        fred_series_id: "VIXCLS",
        indicator_id: "VIX",
        indicator_name: "VIX",
        indicator_kind: "volatility",
    },
    SeriesDef {
        fred_series_id: "DGS10",
        indicator_id: "US10Y",
        indicator_name: "米10年債利回り",
        indicator_kind: "rate",
    },
    SeriesDef {
        fred_series_id: "NIKKEI225",
        indicator_id: "NIKKEI225",
        indicator_name: "日経225",
        indicator_kind: "index",
    },
];

/// 1 系列 1 サイクルの取り込み結果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub upserted: usize,
}

/// `indicator` 行が無ければ作る。既存行は上書きしない。
async fn ensure_indicator(
    db: &impl sea_orm::ConnectionTrait,
    def: &SeriesDef,
) -> Result<(), AppError> {
    indicator::Entity::insert(indicator::ActiveModel {
        id: Set(def.indicator_id.to_string()),
        name: Set(def.indicator_name.to_string()),
        kind: Set(def.indicator_kind.to_string()),
    })
    .on_conflict(
        OnConflict::column(indicator::Column::Id)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

/// 格納済みの最新観測日を返す。1 件も無ければ `None`。
async fn find_latest_observation_date(
    db: &impl sea_orm::ConnectionTrait,
    indicator_id: &str,
) -> Result<Option<NaiveDate>, AppError> {
    let latest = indicator_observation::Entity::find()
        .filter(indicator_observation::Column::IndicatorId.eq(indicator_id))
        .order_by_desc(indicator_observation::Column::Date)
        .one(db)
        .await?;
    Ok(latest.map(|row| row.date))
}

/// 観測値を `(indicator_id, date)` で upsert する。既存行は値を上書きする (FRED の確定値
/// 反映を取りこぼさないため)。
async fn upsert_observations(
    db: &impl sea_orm::ConnectionTrait,
    indicator_id: &str,
    observations: Vec<IndicatorObservation>,
) -> Result<usize, AppError> {
    if observations.is_empty() {
        return Ok(0);
    }
    let count = observations.len();
    let models: Vec<indicator_observation::ActiveModel> = observations
        .into_iter()
        .map(|obs| indicator_observation::ActiveModel {
            indicator_id: Set(indicator_id.to_string()),
            date: Set(obs.date),
            value: Set(obs.value),
        })
        .collect();

    indicator_observation::Entity::insert_many(models)
        .on_conflict(
            OnConflict::columns([
                indicator_observation::Column::IndicatorId,
                indicator_observation::Column::Date,
            ])
            .update_column(indicator_observation::Column::Value)
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(count)
}

/// 1 系列を 1 サイクル分取り込む。初回 (格納済みデータが無い) は全履歴を取得する。
async fn run_ingest_cycle(
    db: &impl sea_orm::ConnectionTrait,
    source: &dyn IndicatorObservationSource,
    def: &SeriesDef,
) -> Result<IngestStats, FredIngestError> {
    ensure_indicator(db, def).await?;

    let latest = find_latest_observation_date(db, def.indicator_id).await?;
    let observation_start = latest.map(|d| d - chrono::Duration::days(LOOKBACK_DAYS));

    let observations = source
        .fetch_observations(def.fred_series_id, observation_start)
        .await?;
    let upserted = upsert_observations(db, def.indicator_id, observations).await?;

    Ok(IngestStats { upserted })
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedIndicatorObservationSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            for def in SERIES {
                match run_ingest_cycle(&db, source.as_ref(), def).await {
                    Ok(stats) => {
                        tracing::debug!(
                            series = def.fred_series_id,
                            upserted = stats.upserted,
                            "FRED ingest cycle completed",
                        );
                    }
                    Err(err) => {
                        tracing::warn!(series = def.fred_series_id, %err, "FRED ingest cycle failed");
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use gateway_fred::FredClient;
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use sqlx::PgPool;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::testing::create_test_db;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn observations_body(rows: &[(&str, &str)]) -> serde_json::Value {
        serde_json::json!({
            "observations": rows.iter().map(|(date, value)| serde_json::json!({
                "date": date,
                "value": value,
            })).collect::<Vec<_>>(),
        })
    }

    async fn mount_series(
        server: &MockServer,
        series_id: &str,
        observation_start: Option<&str>,
        rows: &[(&str, &str)],
    ) {
        let mut mock = Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("series_id", series_id));
        mock = match observation_start {
            Some(start) => mock.and(query_param("observation_start", start)),
            None => mock,
        };
        mock.respond_with(ResponseTemplate::new(200).set_body_json(observations_body(rows)))
            .mount(server)
            .await;
    }

    fn series_def() -> SeriesDef {
        SeriesDef {
            fred_series_id: "DEXJPUS",
            indicator_id: "USDJPY",
            indicator_name: "ドル円",
            indicator_kind: "fx",
        }
    }

    #[backend_test_macros::database_test]
    async fn creates_indicator_and_ingests_full_history_when_table_is_empty(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = MockServer::start().await;
        mount_series(
            &server,
            "DEXJPUS",
            None,
            &[("2026-09-01", "147.50"), ("2026-09-02", ".")],
        )
        .await;
        let client =
            FredClient::with_base_url("test-key".to_string(), &server.uri()).expect("client");

        let stats = run_ingest_cycle(&db, &client, &series_def())
            .await
            .expect("cycle ok");

        assert_eq!(stats, IngestStats { upserted: 1 });

        let ind = indicator::Entity::find_by_id("USDJPY".to_string())
            .one(&db)
            .await
            .expect("query ok")
            .expect("indicator row exists");
        assert_eq!(
            ind,
            indicator::Model {
                id: "USDJPY".to_string(),
                name: "ドル円".to_string(),
                kind: "fx".to_string(),
            }
        );

        let obs =
            indicator_observation::Entity::find_by_id(("USDJPY".to_string(), date(2026, 9, 1)))
                .one(&db)
                .await
                .expect("query ok")
                .expect("observation row exists");
        assert_eq!(obs.value, rust_decimal::Decimal::new(14750, 2));
    }

    #[backend_test_macros::database_test]
    async fn resumes_from_latest_date_minus_lookback_and_updates_existing_value(pool: PgPool) {
        let db = create_test_db(pool).await;
        let def = series_def();

        indicator::ActiveModel {
            id: Set(def.indicator_id.to_string()),
            name: Set(def.indicator_name.to_string()),
            kind: Set(def.indicator_kind.to_string()),
        }
        .insert(&db)
        .await
        .expect("seed indicator");
        indicator_observation::ActiveModel {
            indicator_id: Set(def.indicator_id.to_string()),
            date: Set(date(2026, 9, 1)),
            value: Set(rust_decimal::Decimal::new(14700, 2)),
        }
        .insert(&db)
        .await
        .expect("seed observation");

        let server = MockServer::start().await;
        let expected_start = (date(2026, 9, 1) - chrono::Duration::days(LOOKBACK_DAYS))
            .format("%Y-%m-%d")
            .to_string();
        mount_series(
            &server,
            "DEXJPUS",
            Some(&expected_start),
            &[("2026-09-01", "147.50")],
        )
        .await;
        let client =
            FredClient::with_base_url("test-key".to_string(), &server.uri()).expect("client");

        let stats = run_ingest_cycle(&db, &client, &def)
            .await
            .expect("cycle ok");

        assert_eq!(stats, IngestStats { upserted: 1 });
        let obs = indicator_observation::Entity::find_by_id((
            def.indicator_id.to_string(),
            date(2026, 9, 1),
        ))
        .one(&db)
        .await
        .expect("query ok")
        .expect("observation row exists");
        assert_eq!(obs.value, rust_decimal::Decimal::new(14750, 2));
    }

    #[backend_test_macros::database_test]
    async fn does_not_overwrite_existing_indicator_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let def = series_def();
        indicator::ActiveModel {
            id: Set(def.indicator_id.to_string()),
            name: Set("カスタム名".to_string()),
            kind: Set("custom".to_string()),
        }
        .insert(&db)
        .await
        .expect("seed indicator");

        let server = MockServer::start().await;
        mount_series(&server, "DEXJPUS", None, &[]).await;
        let client =
            FredClient::with_base_url("test-key".to_string(), &server.uri()).expect("client");

        run_ingest_cycle(&db, &client, &def)
            .await
            .expect("cycle ok");

        let ind = indicator::Entity::find_by_id(def.indicator_id.to_string())
            .one(&db)
            .await
            .expect("query ok")
            .expect("indicator row exists");
        assert_eq!(
            ind,
            indicator::Model {
                id: "USDJPY".to_string(),
                name: "カスタム名".to_string(),
                kind: "custom".to_string(),
            }
        );
    }
}
