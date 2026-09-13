//! J-Quants の「日付だけ指定して全銘柄/全業種分を取得する」形のエンドポイントを
//! 日次でバックフィル・再取得する共通ロジック。
//! `short_sale_report_ingest`/`short_ratio_ingest` から使われる。

use std::future::Future;

use chrono::{Duration, NaiveDate, Utc};
use sea_orm::DatabaseConnection;

use crate::data_provider::DataProviderError;
use crate::data_provider::jquants::JQuantsClient;
use crate::error::AppError;
use crate::models::jquants_plan::JQuantsPlan;

/// 既存データの再取得で遡る日数。J-Quants は差分取得非対応で訂正が上書き反映されるため、
/// 直近の訂正を拾えるよう毎サイクル少し遡って取り直す。この日数より前まで遡る訂正
/// (例: 全期間の数値を一括で正規化する訂正) は次サイクルでは拾われない。
pub(crate) const CATCH_UP_LOOKBACK_DAYS: i64 = 7;

/// poll サイクルの結果統計
///
/// `short_sale_report_ingest`/`short_ratio_ingest` の `run_ingest_cycle` (pub) が
/// そのまま返すため pub にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DailyIngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}

/// 日付だけを指定して全銘柄/全業種分を取得する J-Quants エンドポイントの取り込み対象。
/// `short_sale_report_ingest`/`short_ratio_ingest` がそれぞれの型に対して実装する。
///
/// `spawn_poll` が `tokio::spawn` (`Send` 要求) の中で使うため、各メソッドの戻り値は
/// `async fn` ではなく明示的に `+ Send` を付けた `impl Future` で宣言する。実装側は
/// 通常どおり `async fn` で書ける。
pub(crate) trait DailyJQuantsIngest: Sized {
    /// エンドポイントのデータ提供開始日
    const START_DATE: NaiveDate;

    fn fetch(
        client: &JQuantsClient,
        day: NaiveDate,
    ) -> impl Future<Output = Result<Vec<Self>, DataProviderError>> + Send;
    fn upsert(
        db: &DatabaseConnection,
        items: Vec<Self>,
    ) -> impl Future<Output = Result<(), AppError>> + Send;
    fn find_latest_date(
        db: &DatabaseConnection,
    ) -> impl Future<Output = Result<Option<NaiveDate>, AppError>> + Send;
}

/// `T` を DB 上の最新日から訂正分を遡った日付から当日まで、日ごとに 1 リクエストずつ
/// 取得して upsert する。
///
/// 取得可能な日数に上限は設けない — sector_backfill.rs の `BATCH_LIMIT` とは異なり、
/// リクエストは `JQuantsClient` の共有 `RateLimiter` によって直列化されるため、長時間の
/// 初回バックフィル中も他ジョブ (チャート表示の株価取得等) は自分の順番が来るまで
/// 待つだけで済み、専有にはならない。
///
/// `T` は Standard 以上のプランでのみ提供されるデータであることを前提にしている
/// (未満のプランではスキップする)。
pub(crate) async fn run_ingest_cycle<T: DailyJQuantsIngest>(
    db: &DatabaseConnection,
    client: &JQuantsClient,
) -> Result<DailyIngestStats, DataProviderError> {
    let mut stats = DailyIngestStats::default();

    let Some(plan) = client.manual_plan() else {
        tracing::debug!("契約プラン未設定のため取り込みをスキップ");
        return Ok(stats);
    };
    if !matches!(plan, JQuantsPlan::Standard | JQuantsPlan::Premium) {
        tracing::debug!(
            ?plan,
            "このデータは Standard 以上のプランが必要なためスキップ"
        );
        return Ok(stats);
    }

    let today = Utc::now().date_naive();
    let floor = plan.range(today).0.max(T::START_DATE);
    let latest = T::find_latest_date(db)
        .await
        .map_err(|e| DataProviderError::Database(e.to_string()))?;
    let from = latest
        .map(|d| (d - Duration::days(CATCH_UP_LOOKBACK_DAYS)).max(floor))
        .unwrap_or(floor);

    let mut day = from;
    while day <= today {
        match T::fetch(client, day).await {
            Ok(items) => {
                stats.days_fetched += 1;
                if !items.is_empty() {
                    stats.rows_upserted += items.len();
                    T::upsert(db, items)
                        .await
                        .map_err(|e| DataProviderError::Database(e.to_string()))?;
                }
            }
            Err(err) => {
                // 次サイクルは DB 上の最新日付から遡って再開するため、失敗日をスキップして
                // 先に進めると、後続日が成功して最新日付が進んだ時点でこの日が二度と再試行
                // されなくなる。そのため continue ではなく break で打ち切り、同じ from から
                // 再開させる。
                tracing::warn!(%day, %err, "取得に失敗、サイクルを打ち切り");
                break;
            }
        }
        day += Duration::days(1);
    }

    Ok(stats)
}

/// poll task を起動する共通ヘルパー。1 回目は即実行し、その後 `interval` で繰り返す。
pub(crate) fn spawn_poll<T: DailyJQuantsIngest + Send + 'static>(
    db: DatabaseConnection,
    provider: std::sync::Arc<crate::data_provider::DataProviderKind>,
    interval: std::time::Duration,
    label: &'static str,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let crate::data_provider::DataProviderKind::JQuants(client) = provider.as_ref() else {
                tracing::warn!(
                    label,
                    "J-Quants 以外の DataProvider のため取り込みをスキップ"
                );
                continue;
            };
            match run_ingest_cycle::<T>(&db, client).await {
                Ok(stats) => tracing::debug!(
                    label,
                    days_fetched = stats.days_fetched,
                    rows_upserted = stats.rows_upserted,
                    "ingest cycle completed",
                ),
                Err(err) => tracing::warn!(label, %err, "ingest cycle failed"),
            }
        }
    })
}
