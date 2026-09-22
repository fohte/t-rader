//! J-Quants の日次バリュエーション指標を全銘柄分取り込む定期タスク。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::task::JoinHandle;

use crate::data_provider::DataProviderError;
use crate::data_provider::DataProviderKind;
use crate::data_provider::jquants::JQuantsClient;
use crate::date_utils::latest_business_day;
use crate::entities::{jquants_valuation, jquants_valuation_ingested_date};
use crate::error::AppError;
use crate::models::jquants_plan::JQuantsPlan;

/// poll task のデフォルト実行間隔。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 初回取得で欠けを補完する営業日数。
const TARGET_BUSINESS_DAYS: usize = 400;

/// 遡及訂正を拾うため毎回取り直す直近営業日数。
const REFETCH_WINDOW_BUSINESS_DAYS: usize = 7;

/// poll サイクルの結果統計。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_attempted: usize,
    pub rows_upserted: usize,
}

/// `to` を含めて過去に遡り、直近 `count` 営業日を古い順で返す。
fn recent_business_days(to: NaiveDate, count: usize) -> Vec<NaiveDate> {
    let mut days = Vec::with_capacity(count);
    let mut date = to;
    while days.len() < count {
        if latest_business_day(date) == date {
            days.push(date);
        }
        date -= ChronoDuration::days(1);
    }
    days.reverse();
    days
}

/// 直近 `REFETCH_WINDOW_BUSINESS_DAYS` 日と未取り込み日を返す。
fn target_dates(business_days: &[NaiveDate], ingested: &HashSet<NaiveDate>) -> Vec<NaiveDate> {
    let refetch_from_index = business_days
        .len()
        .saturating_sub(REFETCH_WINDOW_BUSINESS_DAYS);
    business_days
        .iter()
        .enumerate()
        .filter(|(index, date)| *index >= refetch_from_index || !ingested.contains(date))
        .map(|(_, date)| *date)
        .collect()
}

/// `from` 以降の取り込み済み営業日を返す。
async fn find_ingested_dates(
    db: &DatabaseConnection,
    from: NaiveDate,
) -> Result<HashSet<NaiveDate>, AppError> {
    let rows = jquants_valuation_ingested_date::Entity::find()
        .filter(jquants_valuation_ingested_date::Column::Date.gte(from))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|row| row.date).collect())
}

/// 取り込み済み営業日を記録する。
async fn mark_ingested(db: &DatabaseConnection, date: NaiveDate) -> Result<(), AppError> {
    jquants_valuation_ingested_date::Entity::insert(jquants_valuation_ingested_date::ActiveModel {
        date: Set(date),
    })
    .on_conflict(
        OnConflict::column(jquants_valuation_ingested_date::Column::Date)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

/// 1 日分の指標を型付きカラムへ upsert する。
async fn upsert_valuations(
    db: &DatabaseConnection,
    items: Vec<crate::data_provider::jquants::ValuationRecord>,
) -> Result<usize, AppError> {
    if items.is_empty() {
        return Ok(0);
    }

    let mut models = Vec::with_capacity(items.len());
    for item in items {
        let date = NaiveDate::parse_from_str(&item.date, "%Y-%m-%d").map_err(|error| {
            AppError::DataProvider(DataProviderError::Parse(format!(
                "invalid valuation date: {error}"
            )))
        })?;
        models.push(jquants_valuation::ActiveModel {
            code: Set(item.code),
            date: Set(date),
            eps: Set(item.eps),
            fwd_eps: Set(item.fwd_eps),
            bps: Set(item.bps),
            roe: Set(item.roe),
            fwd_roe: Set(item.fwd_roe),
            per: Set(item.per),
            fwd_per: Set(item.fwd_per),
            pbr: Set(item.pbr),
            mkt_cap: Set(item.mkt_cap),
        });
    }
    let row_count = models.len();

    jquants_valuation::Entity::insert_many(models)
        .on_conflict(
            OnConflict::columns([
                jquants_valuation::Column::Code,
                jquants_valuation::Column::Date,
            ])
            .update_columns([
                jquants_valuation::Column::Eps,
                jquants_valuation::Column::FwdEps,
                jquants_valuation::Column::Bps,
                jquants_valuation::Column::Roe,
                jquants_valuation::Column::FwdRoe,
                jquants_valuation::Column::Per,
                jquants_valuation::Column::FwdPer,
                jquants_valuation::Column::Pbr,
                jquants_valuation::Column::MktCap,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(row_count)
}

/// valuation を取り込む 1 サイクル。Standard / Premium 以外では何も取得しない。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    client: &JQuantsClient,
) -> Result<IngestStats, AppError> {
    let Some(plan @ (JQuantsPlan::Standard | JQuantsPlan::Premium)) = client.manual_plan() else {
        tracing::debug!(
            "J-Quants Standard 未満または契約プラン未設定のため valuation をスキップします"
        );
        return Ok(IngestStats::default());
    };

    let today = Utc::now().date_naive();
    let (plan_from, plan_to) = plan.range(today);
    let to = latest_business_day(plan_to.min(today));
    let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS)
        .into_iter()
        .filter(|date| *date >= plan_from)
        .collect::<Vec<_>>();
    let Some(&earliest) = business_days.first() else {
        return Ok(IngestStats::default());
    };

    let ingested = find_ingested_dates(db, earliest).await?;
    let targets = target_dates(&business_days, &ingested);
    let mut stats = IngestStats::default();

    for date in targets {
        stats.days_attempted += 1;
        match client.fetch_valuation_by_date(date).await {
            Ok(items) if items.is_empty() => {
                tracing::debug!(%date, "この日の valuation はまだ公開されていません");
            }
            Ok(items) => match upsert_valuations(db, items).await {
                Ok(row_count) => {
                    stats.rows_upserted += row_count;
                    if let Err(error) = mark_ingested(db, date).await {
                        tracing::warn!(%date, %error, "valuation の取り込み日記録に失敗、この日を再試行します");
                    }
                }
                Err(error) => {
                    tracing::warn!(%date, %error, "valuation の格納に失敗、この日をスキップします");
                }
            },
            Err(error) => {
                tracing::warn!(%date, %error, "valuation の取得に失敗、この日をスキップします");
            }
        }
    }

    Ok(stats)
}

/// poll task を起動する。`provider` は J-Quants のときのみ呼び出す。
pub fn spawn_poll(
    db: DatabaseConnection,
    provider: Arc<DataProviderKind>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let DataProviderKind::JQuants(client) = provider.as_ref() else {
            tracing::error!("valuation ingest は J-Quants 専用のため起動できません");
            return;
        };

        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, client).await {
                Ok(stats) => tracing::debug!(
                    days_attempted = stats.days_attempted,
                    rows_upserted = stats.rows_upserted,
                    "valuation ingest cycle completed",
                ),
                Err(error) => tracing::warn!(%error, "valuation ingest cycle failed"),
            }
        }
    })
}
