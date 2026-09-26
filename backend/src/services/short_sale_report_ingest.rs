//! J-Quants `/markets/short-sale-report` (空売り残高報告) を日次で取り込むバックグラウンドタスク。
//!
//! 取得元が取得できる範囲を返さない間はスキップする。日次取り込みの共通ロジックは
//! `jquants_daily_ingest` を参照。

use std::time::Duration;

use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::data_provider::{SharedShortSellingSource, ShortSellingSource, ShortSellingSourceError};
use crate::error::AppError;
use crate::models::ShortSaleReport;
use crate::repositories::short_sale_report::{find_latest_disc_date, upsert_short_sale_reports};
use crate::services::jquants_daily_ingest::{self, DailyIngestStats, DailyJQuantsIngest};

/// エンドポイントのデータ提供開始日 (公式ドキュメント記載)
const SHORT_SALE_REPORT_START_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2013, 11, 7) {
    Some(d) => d,
    None => panic!("invalid constant date"),
};

/// poll task のデフォルト実行間隔。空売り残高報告は日次更新のデータのため、
/// リアルタイム性を重視しないプロダクト方針も踏まえ 1 日間隔にする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

impl DailyJQuantsIngest for ShortSaleReport {
    const START_DATE: NaiveDate = SHORT_SALE_REPORT_START_DATE;

    async fn fetch(
        source: &dyn ShortSellingSource,
        day: NaiveDate,
    ) -> Result<Vec<Self>, ShortSellingSourceError> {
        source.fetch_short_sale_reports(day).await
    }

    async fn upsert(db: &impl sea_orm::ConnectionTrait, items: Vec<Self>) -> Result<(), AppError> {
        upsert_short_sale_reports(db, items).await
    }

    async fn find_latest_date(
        db: &impl sea_orm::ConnectionTrait,
    ) -> Result<Option<NaiveDate>, AppError> {
        find_latest_disc_date(db).await
    }
}

/// 空売り残高報告を DB 上の最新公表日から訂正分を遡った日付から当日まで、
/// 日ごとに 1 リクエストずつ取得して upsert する。
pub async fn run_ingest_cycle(
    db: &impl sea_orm::ConnectionTrait,
    source: &dyn ShortSellingSource,
) -> Result<DailyIngestStats, AppError> {
    jquants_daily_ingest::run_ingest_cycle::<ShortSaleReport>(db, source).await
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedShortSellingSource,
    interval: Duration,
) -> JoinHandle<()> {
    jquants_daily_ingest::spawn_poll::<ShortSaleReport>(
        db,
        source,
        interval,
        "short sale report ingest",
    )
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::{DatabaseBackend, EntityTrait, MockDatabase};
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockShortSaleReport};
    use crate::entities::short_sale_report;
    use crate::models::jquants_plan::JQuantsPlan;
    use crate::testing::create_test_db;

    fn sample_report(ratio: f64) -> MockShortSaleReport {
        MockShortSaleReport {
            code: "7203",
            ss_name: "テスト証券",
            short_position_ratio: ratio,
            prev_report_date: "",
        }
    }

    fn make_report(disc_date: NaiveDate, ratio: f64) -> ShortSaleReport {
        ShortSaleReport {
            disc_date,
            calc_date: disc_date,
            code: "7203".to_string(),
            ss_name: "テスト証券".to_string(),
            ss_addr: "テスト住所".to_string(),
            dic_name: "テスト委託者".to_string(),
            dic_addr: "テスト住所".to_string(),
            fund_name: String::new(),
            short_position_ratio: Decimal::try_from(ratio).expect("decimal"),
            short_position_shares: 1000,
            short_position_units: 10,
            prev_report_date: None,
            prev_report_ratio: None,
            notes: String::new(),
        }
    }

    /// `date` へのリクエストのみ成功させ、それ以外の日付は即エラー (403、リトライなし) を
    /// 返すモックを組み立てる。ループが `date` から始まることを、1 日目で成功し
    /// 2 日目で打ち切られる挙動を通じて検証するために使う。
    async fn mock_succeeds_once_then_fails(mock: &JQuantsMockServer, date: NaiveDate, ratio: f64) {
        mock.short_sale_report()
            .disc_date(&date.format("%Y-%m-%d").to_string())
            .reports(vec![sample_report(ratio)])
            .ok()
            .await;
        mock.error().forbidden("/markets/short-sale-report").await;
    }

    #[sqlx::test(migrations = false)]
    async fn backfills_from_endpoint_start_date_when_db_is_empty(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, SHORT_SALE_REPORT_START_DATE, 0.05).await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Premium));

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
        let latest = find_latest_disc_date(&db).await.expect("query ok");
        assert_eq!(latest, Some(SHORT_SALE_REPORT_START_DATE));
    }

    #[sqlx::test(migrations = false)]
    async fn resumes_from_latest_disc_date_minus_lookback(pool: PgPool) {
        let db = create_test_db(pool).await;
        let latest = Utc::now().date_naive() - chrono::Duration::days(365);
        upsert_short_sale_reports(&db, vec![make_report(latest, 0.05)])
            .await
            .expect("seed");

        let expected_from =
            latest - chrono::Duration::days(jquants_daily_ingest::CATCH_UP_LOOKBACK_DAYS);
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, expected_from, 0.06).await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Premium));

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
        let seeded_row = short_sale_report::Entity::find_by_id((
            expected_from,
            expected_from,
            "7203".to_string(),
            "テスト証券".to_string(),
            "テスト住所".to_string(),
            "テスト委託者".to_string(),
            "テスト住所".to_string(),
            String::new(),
        ))
        .one(&db)
        .await
        .expect("query ok");
        assert_eq!(
            seeded_row.map(|r| r.short_position_ratio),
            Some(Decimal::try_from(0.06).expect("decimal"))
        );
    }

    #[sqlx::test(migrations = false)]
    async fn resume_date_is_clamped_to_endpoint_start_date(pool: PgPool) {
        let db = create_test_db(pool).await;
        // latest - lookback がエンドポイント開始日より前になるケース
        let latest = SHORT_SALE_REPORT_START_DATE + chrono::Duration::days(1);
        upsert_short_sale_reports(&db, vec![make_report(latest, 0.05)])
            .await
            .expect("seed");

        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, SHORT_SALE_REPORT_START_DATE, 0.07).await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Premium));

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            DailyIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn standard_plan_is_also_accepted_by_the_plan_gate(pool: PgPool) {
        let db = create_test_db(pool).await;
        let today = Utc::now().date_naive();
        let floor = JQuantsPlan::Standard
            .range(today)
            .0
            .max(SHORT_SALE_REPORT_START_DATE);
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, floor, 0.05).await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

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
    #[case::unset(None)]
    #[case::free(Some(JQuantsPlan::Free))]
    #[case::light(Some(JQuantsPlan::Light))]
    #[tokio::test]
    async fn skips_fetching_when_plan_is_not_standard_or_above(#[case] plan: Option<JQuantsPlan>) {
        // DB へのクエリが実際に発行されたら (ガードが機能していなければ) 未設定の
        // クエリ結果を求めてエラーになる想定で、DB に触れないことを間接的に検証する
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        if let Some(plan) = plan {
            client.set_manual_plan(Some(plan));
        }

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(stats, DailyIngestStats::default());
    }

    #[sqlx::test(migrations = false)]
    async fn overwrites_existing_row_on_correction(pool: PgPool) {
        let db = create_test_db(pool).await;
        let target_date = SHORT_SALE_REPORT_START_DATE;

        let first_mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&first_mock, target_date, 0.05).await;
        let first_client = first_mock.client().expect("client");
        first_client.set_manual_plan(Some(JQuantsPlan::Premium));
        run_ingest_cycle(&db, &first_client)
            .await
            .expect("first cycle ok");

        // 同じ disc_date への 2 回目の取得。訂正 (割合の変化) を模す
        let second_mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&second_mock, target_date, 0.09).await;
        let second_client = second_mock.client().expect("client");
        second_client.set_manual_plan(Some(JQuantsPlan::Premium));
        run_ingest_cycle(&db, &second_client)
            .await
            .expect("second cycle ok");

        let rows = short_sale_report::Entity::find()
            .all(&db)
            .await
            .expect("find failed");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].short_position_ratio,
            Decimal::try_from(0.09).expect("decimal")
        );
    }
}
