//! J-Quants `/markets/short-ratio` (業種別空売り比率) を日次で取り込むバックグラウンドタスク。
//!
//! 取得元が取得できる範囲を返さない間はスキップする。日次取り込みの共通ロジックは
//! `jquants_daily_ingest` を参照。

use std::time::Duration;

use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::data_provider::{SharedShortSellingSource, ShortSellingSource, ShortSellingSourceError};
use crate::error::AppError;
use crate::models::ShortRatio;
use crate::repositories::short_ratio::{find_latest_date, upsert_short_ratios};
use crate::services::jquants_daily_ingest::{self, DailyIngestStats, DailyJQuantsIngest};

/// エンドポイントのデータ提供開始日 (公式ドキュメント記載)
const SHORT_RATIO_START_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2008, 11, 5) {
    Some(d) => d,
    None => panic!("invalid constant date"),
};

/// poll task のデフォルト実行間隔。業種別空売り比率は日次更新のデータのため、
/// リアルタイム性を重視しないプロダクト方針も踏まえ 1 日間隔にする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

impl DailyJQuantsIngest for ShortRatio {
    const START_DATE: NaiveDate = SHORT_RATIO_START_DATE;

    async fn fetch(
        source: &dyn ShortSellingSource,
        day: NaiveDate,
    ) -> Result<Vec<Self>, ShortSellingSourceError> {
        source.fetch_short_ratios(day).await
    }

    async fn upsert(db: &impl sea_orm::ConnectionTrait, items: Vec<Self>) -> Result<(), AppError> {
        upsert_short_ratios(db, items).await
    }

    async fn find_latest_date(
        db: &impl sea_orm::ConnectionTrait,
    ) -> Result<Option<NaiveDate>, AppError> {
        find_latest_date(db).await
    }
}

/// 業種別空売り比率を DB 上の最新対象日から訂正分を遡った日付から当日まで、
/// 日ごとに 1 リクエストずつ取得して upsert する。
pub async fn run_ingest_cycle(
    db: &impl sea_orm::ConnectionTrait,
    source: &dyn ShortSellingSource,
) -> Result<DailyIngestStats, AppError> {
    jquants_daily_ingest::run_ingest_cycle::<ShortRatio>(db, source).await
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedShortSellingSource,
    interval: Duration,
) -> JoinHandle<()> {
    jquants_daily_ingest::spawn_poll::<ShortRatio>(db, source, interval, "short ratio ingest")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockShortRatio};
    use crate::entities::short_ratio;
    use crate::models::jquants_plan::JQuantsPlan;
    use chrono::Utc;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::{DatabaseBackend, EntityTrait, MockDatabase};
    fn sample_ratio(value: f64) -> MockShortRatio {
        MockShortRatio {
            sector33_code: "0050",
            sell_excluding_short_value: Some(value),
        }
    }

    fn make_ratio(date: NaiveDate, value: f64) -> ShortRatio {
        ShortRatio {
            date,
            sector33_code: "0050".to_string(),
            sell_excluding_short_value: Some(Decimal::try_from(value).expect("decimal")),
            short_with_restriction_value: Some(Decimal::try_from(value).expect("decimal")),
            short_without_restriction_value: Some(Decimal::try_from(value).expect("decimal")),
        }
    }

    /// `date` へのリクエストのみ成功させ、それ以外の日付は即エラー (403、リトライなし) を
    /// 返すモックを組み立てる。ループが `date` から始まることを、1 日目で成功し
    /// 2 日目で打ち切られる挙動を通じて検証するために使う。
    async fn mock_succeeds_once_then_fails(mock: &JQuantsMockServer, date: NaiveDate, value: f64) {
        mock.short_ratio()
            .date(&date.format("%Y-%m-%d").to_string())
            .ratios(vec![sample_ratio(value)])
            .ok()
            .await;
        mock.error().forbidden("/markets/short-ratio").await;
    }

    #[backend_test_macros::database_test]
    async fn backfills_from_endpoint_start_date_when_db_is_empty(
        db: crate::database::DatabaseHandle,
    ) {
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, SHORT_RATIO_START_DATE, 100.0).await;
        let client = mock.client_with_plan(JQuantsPlan::Premium).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
        let latest = find_latest_date(&db).await.expect("query ok");
        assert_eq!(latest, Some(SHORT_RATIO_START_DATE));
    }

    #[backend_test_macros::database_test]
    async fn resumes_from_latest_date_minus_lookback(db: crate::database::DatabaseHandle) {
        let latest = Utc::now().date_naive() - chrono::Duration::days(365);
        upsert_short_ratios(&db, vec![make_ratio(latest, 100.0)])
            .await
            .expect("seed");

        let expected_from =
            latest - chrono::Duration::days(jquants_daily_ingest::CATCH_UP_LOOKBACK_DAYS);
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, expected_from, 200.0).await;
        let client = mock.client_with_plan(JQuantsPlan::Premium).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
        let seeded_row = short_ratio::Entity::find_by_id((expected_from, "0050".to_string()))
            .one(&db)
            .await
            .expect("query ok");
        assert_eq!(
            seeded_row.map(|r| r.sell_excluding_short_value),
            Some(Some(Decimal::try_from(200.0).expect("decimal")))
        );
    }

    #[backend_test_macros::database_test]
    async fn resume_date_is_clamped_to_endpoint_start_date(db: crate::database::DatabaseHandle) {
        // latest - lookback がエンドポイント開始日より前になるケース
        let latest = SHORT_RATIO_START_DATE + chrono::Duration::days(1);
        upsert_short_ratios(&db, vec![make_ratio(latest, 100.0)])
            .await
            .expect("seed");

        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, SHORT_RATIO_START_DATE, 300.0).await;
        let client = mock.client_with_plan(JQuantsPlan::Premium).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn standard_plan_is_also_accepted_by_the_plan_gate(db: crate::database::DatabaseHandle) {
        let today = Utc::now().date_naive();
        let floor = JQuantsPlan::Standard
            .range(today)
            .0
            .max(SHORT_RATIO_START_DATE);
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, floor, 100.0).await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
    }

    #[rstest]
    #[case::free(JQuantsPlan::Free)]
    #[case::light(JQuantsPlan::Light)]
    #[tokio::test]
    async fn skips_fetching_when_plan_is_not_standard_or_above(#[case] plan: JQuantsPlan) {
        // DB モックにはクエリ結果を登録していないため、取得対象外なら DB に触れないことを検証する
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let mock = JQuantsMockServer::start().await;
        let client = mock.client_with_plan(plan).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(stats, DailyIngestStats::default());
    }

    #[backend_test_macros::database_test]
    async fn overwrites_existing_row_on_correction(db: crate::database::DatabaseHandle) {
        let target_date = SHORT_RATIO_START_DATE;

        let first_mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&first_mock, target_date, 100.0).await;
        let first_client = first_mock
            .client_with_plan(JQuantsPlan::Premium)
            .expect("client");
        run_ingest_cycle(&db, &first_client)
            .await
            .expect("first cycle ok");

        // 同じ date への 2 回目の取得。訂正 (値の変化) を模す
        let second_mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&second_mock, target_date, 999.0).await;
        let second_client = second_mock
            .client_with_plan(JQuantsPlan::Premium)
            .expect("client");
        run_ingest_cycle(&db, &second_client)
            .await
            .expect("second cycle ok");

        let rows = short_ratio::Entity::find()
            .all(&db)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].sell_excluding_short_value,
            Some(Decimal::try_from(999.0).expect("decimal"))
        );
    }
}
