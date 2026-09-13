//! J-Quants `/markets/short-sale-report` (空売り残高報告) を日次で取り込むバックグラウンドタスク。
//!
//! Standard 以上のプランでのみ提供されるデータのため、契約プランがそれ未満の間は
//! スキップする。日ごとに 1 リクエストずつ順に取得し、DB 上の最新公表日から
//! 再開することで重複取得を避ける。

use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::data_provider::jquants::JQuantsClient;
use crate::data_provider::{DataProviderError, DataProviderKind};
use crate::models::jquants_plan::JQuantsPlan;
use crate::repositories::short_sale_report::{find_latest_disc_date, upsert_short_sale_reports};

/// エンドポイントのデータ提供開始日 (公式ドキュメント記載)
const SHORT_SALE_REPORT_START_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2013, 11, 7) {
    Some(d) => d,
    None => panic!("invalid constant date"),
};

/// 既存データの再取得で遡る日数。J-Quants は差分取得非対応で訂正が上書き反映されるため、
/// 直近の訂正を拾えるよう毎サイクル少し遡って取り直す。この日数より前まで遡る訂正
/// (例: 全期間の数値を一括で正規化する訂正) は次サイクルでは拾われない。
const CATCH_UP_LOOKBACK_DAYS: i64 = 7;

/// poll task のデフォルト実行間隔。空売り残高報告は日次更新のデータのため、
/// リアルタイム性を重視しないプロダクト方針も踏まえ 1 日間隔にする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// poll サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShortSaleReportIngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}

/// 空売り残高報告を DB 上の最新公表日の翌日以降 (訂正を拾うため少し遡る) から
/// 当日まで、日ごとに 1 リクエストずつ取得して upsert する。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    client: &JQuantsClient,
) -> Result<ShortSaleReportIngestStats, DataProviderError> {
    let mut stats = ShortSaleReportIngestStats::default();

    let Some(plan) = client.manual_plan() else {
        tracing::debug!("契約プラン未設定のため空売り残高報告の取り込みをスキップ");
        return Ok(stats);
    };
    if !matches!(plan, JQuantsPlan::Standard | JQuantsPlan::Premium) {
        tracing::debug!(
            ?plan,
            "空売り残高報告は Standard 以上のプランが必要なためスキップ"
        );
        return Ok(stats);
    }

    let today = Utc::now().date_naive();
    let floor = plan.range(today).0.max(SHORT_SALE_REPORT_START_DATE);
    let latest = find_latest_disc_date(db)
        .await
        .map_err(|e| DataProviderError::Database(e.to_string()))?;
    let from = latest
        .map(|d| (d - chrono::Duration::days(CATCH_UP_LOOKBACK_DAYS)).max(floor))
        .unwrap_or(floor);

    let mut day = from;
    while day <= today {
        match client.fetch_short_sale_reports(day).await {
            Ok(reports) => {
                stats.days_fetched += 1;
                if !reports.is_empty() {
                    stats.rows_upserted += reports.len();
                    upsert_short_sale_reports(db, reports)
                        .await
                        .map_err(|e| DataProviderError::Database(e.to_string()))?;
                }
            }
            Err(err) => {
                // 次サイクルは DB 上の最新 disc_date から遡って再開するため、失敗日をスキップして
                // 先に進めると、後続日が成功して最新日付が進んだ時点でこの日が二度と再試行されなく
                // なる。そのため continue ではなく break で打ち切り、同じ from から再開させる。
                tracing::warn!(%day, %err, "空売り残高報告の取得に失敗、サイクルを打ち切り");
                break;
            }
        }
        day += chrono::Duration::days(1);
    }

    Ok(stats)
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    db: DatabaseConnection,
    provider: Arc<DataProviderKind>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let DataProviderKind::JQuants(client) = provider.as_ref() else {
                tracing::warn!(
                    "J-Quants 以外の DataProvider のため空売り残高報告の取り込みをスキップ"
                );
                continue;
            };
            match run_ingest_cycle(&db, client).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_fetched = stats.days_fetched,
                        rows_upserted = stats.rows_upserted,
                        "short sale report ingest cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "short sale report ingest cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::{DatabaseBackend, EntityTrait, MockDatabase};
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::mock::{JQuantsMockServer, MockShortSaleReport};
    use crate::entities::short_sale_report;
    use crate::models::ShortSaleReport;
    use crate::testing::create_test_db;

    fn sample_report(ratio: f64) -> MockShortSaleReport {
        MockShortSaleReport {
            code: "7203",
            ss_name: "テスト証券",
            short_position_ratio: ratio,
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
            ShortSaleReportIngestStats {
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

        let expected_from = latest - chrono::Duration::days(CATCH_UP_LOOKBACK_DAYS);
        let mock = JQuantsMockServer::start().await;
        mock_succeeds_once_then_fails(&mock, expected_from, 0.06).await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Premium));

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(
            stats,
            ShortSaleReportIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            }
        );
        let seeded_row = short_sale_report::Entity::find_by_id((
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
            ShortSaleReportIngestStats {
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
            ShortSaleReportIngestStats {
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

        assert_eq!(stats, ShortSaleReportIngestStats::default());
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
