//! 日次バリュエーション指標を全銘柄分取り込む定期タスク。

use std::collections::HashSet;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use core_application::{SharedValuationSource, ValuationSource};
use core_domain::valuation::Valuation;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::task::JoinHandle;

use crate::date_utils::latest_business_day;
use crate::entities::{valuation, valuation_ingested_date};
use crate::error::AppError;

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
    let rows = valuation_ingested_date::Entity::find()
        .filter(valuation_ingested_date::Column::Date.gte(from))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|row| row.date).collect())
}

/// 取り込み済み営業日を記録する。
async fn mark_ingested(db: &DatabaseConnection, date: NaiveDate) -> Result<(), AppError> {
    valuation_ingested_date::Entity::insert(valuation_ingested_date::ActiveModel {
        date: Set(date),
    })
    .on_conflict(
        OnConflict::column(valuation_ingested_date::Column::Date)
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
    items: Vec<Valuation>,
) -> Result<usize, AppError> {
    if items.is_empty() {
        return Ok(0);
    }

    let mut models = Vec::with_capacity(items.len());
    for item in items {
        models.push(valuation::ActiveModel {
            code: Set(item.code),
            date: Set(item.date),
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

    valuation::Entity::insert_many(models)
        .on_conflict(
            OnConflict::columns([valuation::Column::Code, valuation::Column::Date])
                .update_columns([
                    valuation::Column::Eps,
                    valuation::Column::FwdEps,
                    valuation::Column::Bps,
                    valuation::Column::Roe,
                    valuation::Column::FwdRoe,
                    valuation::Column::Per,
                    valuation::Column::FwdPer,
                    valuation::Column::Pbr,
                    valuation::Column::MktCap,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(row_count)
}

/// 1 サイクル実行。データソースが取得可能範囲を返さない場合はスキップする。
pub async fn run_ingest_cycle(
    db: &DatabaseConnection,
    source: &dyn ValuationSource,
) -> Result<IngestStats, AppError> {
    let today = Utc::now().date_naive();
    let Some(range) = source.fetchable_range(today) else {
        tracing::debug!("valuation を取得できないため取り込みをスキップします");
        return Ok(IngestStats::default());
    };

    let to = latest_business_day(range.to.min(today));
    let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS)
        .into_iter()
        .filter(|date| *date >= range.from)
        .collect::<Vec<_>>();
    let Some(&earliest) = business_days.first() else {
        return Ok(IngestStats::default());
    };

    let ingested = find_ingested_dates(db, earliest).await?;
    let targets = target_dates(&business_days, &ingested);
    let mut stats = IngestStats::default();

    for date in targets {
        stats.days_attempted += 1;
        match source.fetch_valuations_by_date(date).await {
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

/// データソースが設定された場合に poll task を起動する。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedValuationSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref()).await {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::NaiveDate;
    use sea_orm::{DatabaseBackend, MockDatabase};

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[tokio::test]
    async fn skips_when_source_has_no_fetchable_range() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        let stats = run_ingest_cycle(&db, &client).await.expect("cycle ok");
        let requests = mock
            .server_ref()
            .received_requests()
            .await
            .expect("recorded requests")
            .into_iter()
            .map(|request| request.url.path().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            (stats, requests),
            (IngestStats::default(), Vec::<String>::new())
        );
    }

    #[test]
    fn selects_recent_refetch_window_and_missing_dates() {
        let mut business_days = Vec::new();
        let mut candidate = date(2099, 1, 1);
        while business_days.len() < 10 {
            if latest_business_day(candidate) == candidate {
                business_days.push(candidate);
            }
            candidate += ChronoDuration::days(1);
        }
        let ingested = HashSet::from([business_days[1], business_days[2]]);

        assert_eq!(
            target_dates(&business_days, &ingested),
            vec![
                business_days[0],
                business_days[3],
                business_days[4],
                business_days[5],
                business_days[6],
                business_days[7],
                business_days[8],
                business_days[9],
            ],
        );
    }
}
