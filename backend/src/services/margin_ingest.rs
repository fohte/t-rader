//! 信用取引週末残高・日々公表信用取引残高を日次で取得し、DB に蓄積する定期タスク。
//!
//! IBKR には対応するデータが無いため DataProvider trait には追加せず、JQuantsClient を
//! 直接使う。契約プランが未設定の間は取り込まない (未設定時のレート制限は 5 req/min のため)。

use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::data_provider::DataProviderKind;
use crate::data_provider::jquants::JQuantsClient;
use crate::error::AppError;
use crate::repositories::{margin_alert, margin_interest};

/// poll task のデフォルト実行間隔 (1 日)。日次更新の J-Quants データに対して十分な頻度。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 信用取引週末残高の提供開始日 (https://jpx-jquants.com/ja/spec/mkt-margin-int)
fn margin_interest_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2012, 2, 10).unwrap_or_default()
}

/// 日々公表信用取引残高の提供開始日 (https://jpx-jquants.com/ja/spec/mkt-margin-alert)
fn margin_alert_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2008, 5, 8).unwrap_or_default()
}

/// 専用のチェックポイントテーブルは持たず、テーブル内の最新日付から再開する。
/// 既にデータがある場合はこの日数分さかのぼって再取得し、J-Quants 側の訂正
/// (上書きで反映される) を拾う。
const REFETCH_LOOKBACK_DAYS: i64 = 30;

/// 取得開始日を決定する。テーブルが空なら `earliest` から、既にデータがあれば
/// 最新日付から `REFETCH_LOOKBACK_DAYS` 日さかのぼった日 (ただし `earliest` 未満にはしない) から。
fn resolve_start_date(latest_stored: Option<NaiveDate>, earliest: NaiveDate) -> NaiveDate {
    match latest_stored {
        None => earliest,
        Some(latest) => (latest - ChronoDuration::days(REFETCH_LOOKBACK_DAYS)).max(earliest),
    }
}

/// 1 サイクルの取り込み結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}

/// `start` から `end` (両端含む) まで日付を 1 日ずつ進め、信用取引週末残高を取得・保存する。
/// 1 日分の取得・保存に失敗しても残りの日付は続行する。
async fn ingest_margin_interest(
    db: &DatabaseConnection,
    client: &JQuantsClient,
    start: NaiveDate,
    end: NaiveDate,
) -> IngestStats {
    let mut stats = IngestStats::default();
    let mut date = start;
    while date <= end {
        match client.fetch_margin_interest(date).await {
            Ok(records) => {
                let count = records.len();
                match margin_interest::upsert_margin_interest(db, records).await {
                    Ok(()) => {
                        stats.days_fetched += 1;
                        stats.rows_upserted += count;
                    }
                    Err(e) => {
                        tracing::warn!(%date, error = %e, "信用取引週末残高の保存に失敗、次の日に進みます");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(%date, error = %e, "信用取引週末残高の取得に失敗、次の日に進みます");
            }
        }
        date += ChronoDuration::days(1);
    }
    stats
}

/// `start` から `end` (両端含む) まで日付を 1 日ずつ進め、日々公表信用取引残高を取得・保存する。
/// 1 日分の取得・保存に失敗しても残りの日付は続行する。
async fn ingest_margin_alert(
    db: &DatabaseConnection,
    client: &JQuantsClient,
    start: NaiveDate,
    end: NaiveDate,
) -> IngestStats {
    let mut stats = IngestStats::default();
    let mut date = start;
    while date <= end {
        match client.fetch_margin_alert(date).await {
            Ok(records) => {
                let count = records.len();
                match margin_alert::upsert_margin_alert(db, records).await {
                    Ok(()) => {
                        stats.days_fetched += 1;
                        stats.rows_upserted += count;
                    }
                    Err(e) => {
                        tracing::warn!(%date, error = %e, "日々公表信用取引残高の保存に失敗、次の日に進みます");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(%date, error = %e, "日々公表信用取引残高の取得に失敗、次の日に進みます");
            }
        }
        date += ChronoDuration::days(1);
    }
    stats
}

/// 1 サイクル実行: margin_interest, margin_alert それぞれ未取得区間を取得・保存する。
/// 契約プランが未設定なら何もしない。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    client: &JQuantsClient,
) -> Result<(IngestStats, IngestStats), AppError> {
    let Some(plan) = client.manual_plan() else {
        tracing::debug!("J-Quants 契約プラン未設定のため、信用残データの取り込みをスキップします");
        return Ok((IngestStats::default(), IngestStats::default()));
    };

    let today = Utc::now().date_naive();
    let (plan_from, _) = plan.range(today);

    let interest_earliest = plan_from.max(margin_interest_start_date());
    let interest_latest = margin_interest::find_latest_margin_interest_date(db).await?;
    let interest_start = resolve_start_date(interest_latest, interest_earliest);
    let interest_stats = ingest_margin_interest(db, client, interest_start, today).await;

    let alert_earliest = plan_from.max(margin_alert_start_date());
    let alert_latest = margin_alert::find_latest_margin_alert_pub_date(db).await?;
    let alert_start = resolve_start_date(alert_latest, alert_earliest);
    let alert_stats = ingest_margin_alert(db, client, alert_start, today).await;

    Ok((interest_stats, alert_stats))
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    provider: Arc<DataProviderKind>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let DataProviderKind::JQuants(client) = provider.as_ref() else {
            tracing::warn!("margin ingest には J-Quants provider が必要なため起動しません");
            return;
        };

        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, client).await {
                Ok((interest_stats, alert_stats)) => {
                    tracing::info!(
                        ?interest_stats,
                        ?alert_stats,
                        "margin ingest cycle completed"
                    );
                }
                Err(e) => {
                    tracing::warn!(%e, "margin ingest cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty_table_uses_earliest(
        None,
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date")
    )]
    #[case::existing_data_looks_back_from_latest(
        Some(NaiveDate::from_ymd_opt(2024, 6, 15).expect("date")),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
        NaiveDate::from_ymd_opt(2024, 5, 16).expect("date")
    )]
    #[case::lookback_does_not_go_before_earliest(
        Some(NaiveDate::from_ymd_opt(2020, 1, 10).expect("date")),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("date")
    )]
    fn resolve_start_date_cases(
        #[case] latest_stored: Option<NaiveDate>,
        #[case] earliest: NaiveDate,
        #[case] expected: NaiveDate,
    ) {
        assert_eq!(resolve_start_date(latest_stored, earliest), expected);
    }
}
