//! `/fins/summary` (財務情報) を日付指定で定期的に取り込む定期タスク。
//!
//! 取得済みかどうかを判定する専用テーブルは持たず、`jquants_fin_summary` に格納済みの
//! 最新開示日から判断する (テーブルが空なら契約プランの取得可能範囲の先頭から取り込む)。
//! J-Quants の訂正は既存データへの上書きで反映され差分取得もできないため、格納済み最新日
//! からさかのぼって再取得することで取りこぼしに備える。

use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder, Set};
use tokio::task::JoinHandle;

use crate::data_provider::DataProviderError;
use crate::data_provider::jquants::JQuantsClient;
use crate::entities::jquants_fin_summary;
use crate::error::AppError;
use crate::models::jquants_plan::JQuantsPlan;

/// poll task のデフォルト実行間隔。財務情報の更新頻度 (日次) に合わせて 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 格納済み最新開示日から開示訂正を取りこぼさないためにさかのぼる日数。
const LOOKBACK_DAYS: i64 = 30;

/// `/fins/summary` のデータ提供開始日 (公式ページ記載)。これより前を取得しても空振りになる。
fn provision_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2008, 7, 7).unwrap_or_default()
}

/// poll サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_attempted: usize,
    pub upserted: usize,
}

/// 契約プランの提供範囲とデータ提供開始日、格納済み最新日 (ルックバック含む) から
/// 取り込み対象の日付範囲 `(from, to)` を決定する。範囲が反転する場合は `None`。
fn fetch_range(
    today: NaiveDate,
    plan: JQuantsPlan,
    latest_stored: Option<NaiveDate>,
) -> Option<(NaiveDate, NaiveDate)> {
    let (plan_from, plan_to) = plan.range(today);
    let range_from = plan_from.max(provision_start_date());
    if range_from > plan_to {
        return None;
    }

    let from = match latest_stored {
        Some(latest) => (latest - chrono::Duration::days(LOOKBACK_DAYS)).max(range_from),
        None => range_from,
    };
    Some((from, plan_to))
}

/// 格納済みの最新開示日を返す。1 件も無ければ `None`。
async fn find_latest_disc_date(db: &DatabaseConnection) -> Result<Option<NaiveDate>, AppError> {
    let latest = jquants_fin_summary::Entity::find()
        .order_by_desc(jquants_fin_summary::Column::DiscDate)
        .one(db)
        .await?;
    Ok(latest.map(|row| row.disc_date))
}

/// レスポンス 1 件から格納に必要なキー項目 (code, disc_no, disc_date) を取り出す
fn extract_key_fields(
    item: &serde_json::Value,
) -> Result<(String, String, NaiveDate), DataProviderError> {
    let get_str = |key: &str| -> Result<&str, DataProviderError> {
        item.get(key).and_then(|v| v.as_str()).ok_or_else(|| {
            DataProviderError::Parse(format!("fin summary response missing '{key}'"))
        })
    };
    let code = get_str("Code")?.to_string();
    let disc_no = get_str("DiscNo")?.to_string();
    let disc_date = NaiveDate::parse_from_str(get_str("DiscDate")?, "%Y-%m-%d")
        .map_err(|e| DataProviderError::Parse(format!("invalid DiscDate: {e}")))?;
    Ok((code, disc_no, disc_date))
}

/// 1 日分のレスポンスを `jquants_fin_summary` に upsert する。(code, disc_no) が同じ行は
/// 上書きする。
async fn upsert_fin_summaries(
    db: &DatabaseConnection,
    items: Vec<serde_json::Value>,
) -> Result<usize, AppError> {
    if items.is_empty() {
        return Ok(0);
    }

    let mut active_models = Vec::with_capacity(items.len());
    for item in items {
        let (code, disc_no, disc_date) =
            extract_key_fields(&item).map_err(AppError::DataProvider)?;
        active_models.push(jquants_fin_summary::ActiveModel {
            code: Set(code),
            disc_no: Set(disc_no),
            disc_date: Set(disc_date),
            raw: Set(item),
        });
    }
    let count = active_models.len();

    jquants_fin_summary::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([
                jquants_fin_summary::Column::Code,
                jquants_fin_summary::Column::DiscNo,
            ])
            .update_columns([
                jquants_fin_summary::Column::DiscDate,
                jquants_fin_summary::Column::Raw,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(count)
}

/// 財務情報を取り込む 1 サイクル。契約プラン未設定の間は取り込まない
/// (未設定時のレートリミットは 5 req/分で、バックフィルに数日かかるため)。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    client: &JQuantsClient,
) -> Result<IngestStats, AppError> {
    let Some(plan) = client.manual_plan() else {
        tracing::debug!("J-Quants 契約プランが未設定のため財務情報の取り込みをスキップします");
        return Ok(IngestStats::default());
    };

    let today = Utc::now().date_naive();
    let latest_stored = find_latest_disc_date(db).await?;
    let Some((from, to)) = fetch_range(today, plan, latest_stored) else {
        tracing::debug!(
            ?plan,
            "契約プランの取得可能範囲がデータ提供開始日に届かないためスキップします"
        );
        return Ok(IngestStats::default());
    };

    let mut stats = IngestStats::default();
    let mut date = from;
    while date <= to {
        if crate::date_utils::latest_business_day(date) == date {
            stats.days_attempted += 1;
            match client.fetch_fin_summary_by_date(date).await {
                Ok(items) => match upsert_fin_summaries(db, items).await {
                    Ok(n) => stats.upserted += n,
                    Err(e) => {
                        tracing::warn!(%date, error = %e, "財務情報の格納に失敗、この日をスキップします");
                    }
                },
                Err(e) => {
                    tracing::warn!(%date, error = %e, "財務情報の取得に失敗、この日をスキップします");
                }
            }
        }
        date += chrono::Duration::days(1);
    }

    Ok(stats)
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
/// J-Quants client が設定された場合に起動する。
pub fn spawn_poll(
    db: DatabaseConnection,
    client: Arc<JQuantsClient>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, &client).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_attempted = stats.days_attempted,
                        upserted = stats.upserted,
                        "fin summary ingest cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "fin summary ingest cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use serde_json::json;
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::testing::create_test_db;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn count_business_days(from: NaiveDate, to: NaiveDate) -> usize {
        let mut count = 0;
        let mut date = from;
        while date <= to {
            if crate::date_utils::latest_business_day(date) == date {
                count += 1;
            }
            date += chrono::Duration::days(1);
        }
        count
    }

    #[rstest]
    #[case::empty_table_starts_at_plan_from(
        JQuantsPlan::Standard,
        None,
        (date(2016, 9, 15), date(2026, 9, 13))
    )]
    #[case::resumes_with_lookback(
        JQuantsPlan::Standard,
        Some(date(2026, 8, 1)),
        (date(2026, 7, 2), date(2026, 9, 13))
    )]
    #[case::lookback_clamped_to_plan_from(
        JQuantsPlan::Standard,
        Some(date(2016, 9, 20)),
        (date(2016, 9, 15), date(2026, 9, 13))
    )]
    #[case::provision_start_date_clamps_premium(
        JQuantsPlan::Premium,
        None,
        (date(2008, 7, 7), date(2026, 9, 13))
    )]
    fn test_fetch_range(
        #[case] plan: JQuantsPlan,
        #[case] latest_stored: Option<NaiveDate>,
        #[case] expected: (NaiveDate, NaiveDate),
    ) {
        let today = date(2026, 9, 13);
        assert_eq!(fetch_range(today, plan, latest_stored), Some(expected));
    }

    #[sqlx::test(migrations = false)]
    async fn test_skips_when_plan_is_unset(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = JQuantsClient::new("test-api-key".to_string()).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(stats, IngestStats::default());
    }

    #[sqlx::test(migrations = false)]
    async fn test_ingests_and_upserts_new_disclosures(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = Utc::now().date_naive();
        let to = crate::date_utils::latest_business_day(today);
        // 格納済み最新日を直近にしておき、ルックバック期間と数日分に絞る
        let seed_disc_date = to - chrono::Duration::days(3);
        seed_fin_summary(&db, "00000", "0", seed_disc_date).await;

        mock.fin_summary()
            .date(&to.format("%Y-%m-%d").to_string())
            .items(vec![json!({
                "DiscDate": to.format("%Y-%m-%d").to_string(),
                "Code": "72030",
                "DiscNo": "1",
                "Sales": "1000000",
            })])
            .ok()
            .await;
        for offset in 1..=3 {
            let d = to - chrono::Duration::days(offset);
            if crate::date_utils::latest_business_day(d) != d {
                continue;
            }
            mock.fin_summary()
                .date(&d.format("%Y-%m-%d").to_string())
                .items(vec![])
                .ok()
                .await;
        }

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        let (range_from, range_to) =
            fetch_range(today, JQuantsPlan::Standard, Some(seed_disc_date)).expect("range");
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: count_business_days(range_from, range_to),
                upserted: 1,
            }
        );

        let row = jquants_fin_summary::Entity::find_by_id(("72030".to_string(), "1".to_string()))
            .one(&db)
            .await
            .expect("query ok")
            .expect("row exists");
        assert_eq!(row.disc_date, to);
        assert_eq!(
            row.raw,
            json!({
                "DiscDate": to.format("%Y-%m-%d").to_string(),
                "Code": "72030",
                "DiscNo": "1",
                "Sales": "1000000",
            })
        );
    }

    #[sqlx::test(migrations = false)]
    async fn test_continues_past_days_that_fail_to_fetch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = Utc::now().date_naive();
        let to = crate::date_utils::latest_business_day(today);
        let seed_disc_date = to - chrono::Duration::days(1);
        seed_fin_summary(&db, "00000", "0", seed_disc_date).await;

        let prev_business_day =
            crate::date_utils::latest_business_day(to - chrono::Duration::days(1));
        // `to` の日は mock を用意しない (マッチせず 404 → fetch エラー) が、サイクル全体は失敗させない
        mock.fin_summary()
            .date(&prev_business_day.format("%Y-%m-%d").to_string())
            .items(vec![json!({
                "DiscDate": prev_business_day.format("%Y-%m-%d").to_string(),
                "Code": "72030",
                "DiscNo": "1",
            })])
            .ok()
            .await;

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        let (range_from, range_to) =
            fetch_range(today, JQuantsPlan::Standard, Some(seed_disc_date)).expect("range");
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: count_business_days(range_from, range_to),
                upserted: 1,
            }
        );
    }

    async fn seed_fin_summary(
        db: &DatabaseConnection,
        code: &str,
        disc_no: &str,
        disc_date: NaiveDate,
    ) {
        jquants_fin_summary::ActiveModel {
            code: Set(code.to_string()),
            disc_no: Set(disc_no.to_string()),
            disc_date: Set(disc_date),
            raw: Set(
                json!({ "Code": code, "DiscNo": disc_no, "DiscDate": disc_date.format("%Y-%m-%d").to_string() }),
            ),
        }
        .insert(db)
        .await
        .expect("seed fin summary");
    }
}
