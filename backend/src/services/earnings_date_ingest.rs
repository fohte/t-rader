//! 決算発表予定日を日付指定で定期的に取り込む定期タスク。
//!
//! 取得済みかどうかを判定する専用テーブルは持たず、`jquants_earnings_date` に格納済みの
//! 最新公表日から判断する (テーブルが空なら取得元の取得可能範囲の先頭から取り込む)。
//! 予定日の変更は新しい公表日の行として返り差分取得もできないため、格納済み最新日
//! からさかのぼって再取得することで取りこぼしに備える。(code, fq_name, pub_date) を
//! 複合主キーとして公表日ごとの行をすべて残し、上書きしない。

use std::time::Duration;

use chrono::{NaiveDate, Utc};
use core_domain::earnings_schedule::EarningsSchedule;
use sea_orm::sea_query::OnConflict;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder, Set};
use tokio::task::JoinHandle;

use crate::data_provider::{DateRange, EarningsScheduleSource, SharedEarningsScheduleSource};
use crate::entities::jquants_earnings_date;
use crate::error::AppError;

/// poll task のデフォルト実行間隔。決算発表予定日の更新頻度 (日次) に合わせて 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 格納済み最新公表日から予定日の変更を取りこぼさないためにさかのぼる日数。
const LOOKBACK_DAYS: i64 = 30;

/// poll サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_attempted: usize,
    pub upserted: usize,
}

/// 取得元の提供範囲と格納済み最新日 (ルックバック含む) から、取り込み対象の
/// 日付範囲 `(from, to)` を決定する。
///
/// 取得元が返した範囲には下限日を追加せず、その全体をバックフィル対象にする。
fn fetch_range(
    source_range: &DateRange,
    latest_stored: Option<NaiveDate>,
) -> (NaiveDate, NaiveDate) {
    let from = match latest_stored {
        Some(latest) => (latest - chrono::Duration::days(LOOKBACK_DAYS)).max(source_range.from),
        None => source_range.from,
    };
    (from, source_range.to)
}

/// 格納済みの最新公表日を返す。1 件も無ければ `None`。
async fn find_latest_pub_date(
    db: &impl sea_orm::ConnectionTrait,
) -> Result<Option<NaiveDate>, AppError> {
    let latest = jquants_earnings_date::Entity::find()
        .order_by_desc(jquants_earnings_date::Column::PubDate)
        .one(db)
        .await?;
    Ok(latest.map(|row| row.pub_date))
}

/// 中立な決算予定を現在の保存形式に変換する。
fn to_active_model(schedule: EarningsSchedule) -> jquants_earnings_date::ActiveModel {
    jquants_earnings_date::ActiveModel {
        code: Set(schedule.code),
        fq_name: Set(schedule.fiscal_quarter_name),
        pub_date: Set(schedule.published_date),
        sch_date: Set(schedule.scheduled_date),
        fye: Set(schedule.fiscal_year_end),
        co_name: Set(schedule.company_name),
        co_name_en: Set(schedule.company_name_en),
    }
}

/// 1 日分の取得結果を `jquants_earnings_date` に upsert する。(code, fq_name, pub_date)
/// が同じ行は上書きする。
async fn upsert_earnings_dates(
    db: &impl sea_orm::ConnectionTrait,
    items: Vec<EarningsSchedule>,
) -> Result<usize, AppError> {
    if items.is_empty() {
        return Ok(0);
    }

    let active_models = items.into_iter().map(to_active_model).collect::<Vec<_>>();
    let count = active_models.len();

    jquants_earnings_date::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([
                jquants_earnings_date::Column::Code,
                jquants_earnings_date::Column::FqName,
                jquants_earnings_date::Column::PubDate,
            ])
            .update_columns([
                jquants_earnings_date::Column::SchDate,
                jquants_earnings_date::Column::Fye,
                jquants_earnings_date::Column::CoName,
                jquants_earnings_date::Column::CoNameEn,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(count)
}

/// 決算発表予定日を取り込む 1 サイクル。
pub async fn run_ingest_cycle(
    db: &impl sea_orm::ConnectionTrait,
    source: &dyn EarningsScheduleSource,
) -> Result<IngestStats, AppError> {
    let today = Utc::now().date_naive();
    let Some(source_range) = source.fetchable_range(today) else {
        tracing::debug!("決算発表予定日の取得可能範囲がないため取り込みをスキップします");
        return Ok(IngestStats::default());
    };

    let latest_stored = find_latest_pub_date(db).await?;
    let (from, to) = fetch_range(&source_range, latest_stored);

    let mut stats = IngestStats::default();
    let mut date = from;
    while date <= to {
        if crate::date_utils::latest_business_day(date) == date {
            stats.days_attempted += 1;
            match source.fetch_earnings_schedules_by_date(date).await {
                Ok(items) => match upsert_earnings_dates(db, items).await {
                    Ok(n) => stats.upserted += n,
                    Err(e) => {
                        tracing::warn!(%date, error = %e, "決算発表予定日の格納に失敗、この日をスキップします");
                    }
                },
                Err(e) => {
                    tracing::warn!(%date, error = %e, "決算発表予定日の取得に失敗、この日をスキップします");
                }
            }
        }
        date += chrono::Duration::days(1);
    }

    Ok(stats)
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedEarningsScheduleSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(
                        days_attempted = stats.days_attempted,
                        upserted = stats.upserted,
                        "earnings date ingest cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "earnings date ingest cycle failed");
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
    use crate::data_provider::jquants::{JQuantsClient, mock::JQuantsMockServer};
    use crate::models::jquants_plan::JQuantsPlan;
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

    fn plan_range(plan: JQuantsPlan, today: NaiveDate) -> DateRange {
        let (from, to) = plan.range(today);
        DateRange { from, to }
    }

    #[rstest]
    #[case::empty_table_starts_at_range_from(
        JQuantsPlan::Standard,
        None,
        (date(2016, 9, 15), date(2026, 9, 13))
    )]
    #[case::resumes_with_lookback(
        JQuantsPlan::Standard,
        Some(date(2026, 8, 1)),
        (date(2026, 7, 2), date(2026, 9, 13))
    )]
    #[case::lookback_clamped_to_range_from(
        JQuantsPlan::Standard,
        Some(date(2016, 9, 20)),
        (date(2016, 9, 15), date(2026, 9, 13))
    )]
    fn test_fetch_range(
        #[case] plan: JQuantsPlan,
        #[case] latest_stored: Option<NaiveDate>,
        #[case] expected: (NaiveDate, NaiveDate),
    ) {
        let today = date(2026, 9, 13);
        let source_range = plan_range(plan, today);
        assert_eq!(fetch_range(&source_range, latest_stored), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn test_skips_when_plan_is_unset(pool: PgPool) {
        let db = create_test_db(pool).await;
        let client = JQuantsClient::new("test-api-key".to_string()).expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        assert_eq!(stats, IngestStats::default());
    }

    #[sqlx::test(migrations = false)]
    async fn test_ingests_and_upserts_new_disclosures_including_undecided_schedule(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        client.set_manual_plan(Some(JQuantsPlan::Standard));

        let today = Utc::now().date_naive();
        let to = crate::date_utils::latest_business_day(today);
        // 格納済み最新日を直近にしておき、取り込み対象の範囲を数日に絞る
        let seed_pub_date = to - chrono::Duration::days(3 + LOOKBACK_DAYS);
        seed_earnings_date(&db, "00000", "FY", seed_pub_date).await;

        mock.earnings_date()
            .date(&to.format("%Y-%m-%d").to_string())
            .items(vec![json!({
                "PubDate": to.format("%Y-%m-%d").to_string(),
                "SchDate": "",
                "FQName": "1Q",
                "FYE": "0331",
                "Code": "72030",
                "CoName": "テスト株式会社",
                "CoNameEn": "Test Corp.",
            })])
            .ok()
            .await;
        for offset in 1..=3 {
            let d = to - chrono::Duration::days(offset);
            if crate::date_utils::latest_business_day(d) != d {
                continue;
            }
            mock.earnings_date()
                .date(&d.format("%Y-%m-%d").to_string())
                .items(vec![])
                .ok()
                .await;
        }

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        let source_range = plan_range(JQuantsPlan::Standard, today);
        let (range_from, range_to) = fetch_range(&source_range, Some(seed_pub_date));
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: count_business_days(range_from, range_to),
                upserted: 1,
            }
        );

        let row =
            jquants_earnings_date::Entity::find_by_id(("72030".to_string(), "1Q".to_string(), to))
                .one(&db)
                .await
                .expect("query ok")
                .expect("row exists");
        assert_eq!(
            row,
            jquants_earnings_date::Model {
                code: "72030".to_string(),
                fq_name: "1Q".to_string(),
                pub_date: to,
                sch_date: None,
                fye: "0331".to_string(),
                co_name: "テスト株式会社".to_string(),
                co_name_en: "Test Corp.".to_string(),
            }
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
        let seed_pub_date = to - chrono::Duration::days(1 + LOOKBACK_DAYS);
        seed_earnings_date(&db, "00000", "FY", seed_pub_date).await;

        let prev_business_day =
            crate::date_utils::latest_business_day(to - chrono::Duration::days(1));
        // `to` の日は mock を用意しない (マッチせず 404 → fetch エラー) が、サイクル全体は失敗させない
        mock.earnings_date()
            .date(&prev_business_day.format("%Y-%m-%d").to_string())
            .items(vec![json!({
                "PubDate": prev_business_day.format("%Y-%m-%d").to_string(),
                "SchDate": "2026-11-10",
                "FQName": "2Q",
                "FYE": "0331",
                "Code": "72030",
                "CoName": "テスト株式会社",
                "CoNameEn": "Test Corp.",
            })])
            .ok()
            .await;

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");

        let source_range = plan_range(JQuantsPlan::Standard, today);
        let (range_from, range_to) = fetch_range(&source_range, Some(seed_pub_date));
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: count_business_days(range_from, range_to),
                upserted: 1,
            }
        );
    }

    async fn seed_earnings_date(
        db: &impl sea_orm::ConnectionTrait,
        code: &str,
        fq_name: &str,
        pub_date: NaiveDate,
    ) {
        jquants_earnings_date::ActiveModel {
            code: Set(code.to_string()),
            fq_name: Set(fq_name.to_string()),
            pub_date: Set(pub_date),
            sch_date: Set(None),
            fye: Set("0331".to_string()),
            co_name: Set("シード株式会社".to_string()),
            co_name_en: Set("Seed Corp.".to_string()),
        }
        .insert(db)
        .await
        .expect("seed earnings date");
    }
}
