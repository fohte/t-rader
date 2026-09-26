//! `/fins/summary` (財務情報) を日付指定で定期的に取り込む定期タスク。
//!
//! 取得済みかどうかを判定する専用テーブルは持たず、財務情報テーブルに格納済みの
//! 最新開示日から判断する (テーブルが空なら取得可能範囲の先頭から取り込む)。
//! 訂正は既存データへの上書きで反映され差分取得もできないため、格納済み最新日
//! からさかのぼって再取得することで取りこぼしに備える。

use std::time::Duration;

use chrono::{NaiveDate, Utc};
use core_domain::financial_summary::FinancialSummary;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, EntityTrait, Iterable, QueryOrder, Set, TransactionSession,
};
use tokio::task::JoinHandle;

use crate::data_provider::{DateRange, FinancialSummarySource, SharedFinancialSummarySource};
use crate::entities::financial_summary;
use crate::error::AppError;

/// poll task のデフォルト実行間隔。財務情報の更新頻度 (日次) に合わせて 1 日とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 格納済み最新開示日から訂正を取りこぼさないためにさかのぼる日数。
const LOOKBACK_DAYS: i64 = 30;
const MAX_UPSERT_ROWS_PER_STATEMENT: usize = 1_000;

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

/// 提供範囲とデータ提供開始日、格納済み最新日 (ルックバック含む) から取り込み対象の日付範囲を決める。
fn fetch_range(
    range: DateRange,
    latest_stored: Option<NaiveDate>,
) -> Option<(NaiveDate, NaiveDate)> {
    let range_from = range.from.max(provision_start_date());
    if range_from > range.to {
        return None;
    }

    let from = match latest_stored {
        Some(latest) => (latest - chrono::Duration::days(LOOKBACK_DAYS)).max(range_from),
        None => range_from,
    };
    Some((from, range.to))
}

/// 格納済みの最新開示日を返す。1 件も無ければ `None`。
async fn find_latest_disc_date(db: &impl ConnectionTrait) -> Result<Option<NaiveDate>, AppError> {
    let latest = financial_summary::Entity::find()
        .order_by_desc(financial_summary::Column::DisclosureDate)
        .one(db)
        .await?;
    Ok(latest.map(|row| row.disclosure_date))
}

/// 1 日分の財務情報を upsert する。同じ銘柄・開示番号の訂正は既存行を上書きする。
async fn upsert_fin_summaries(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    items: Vec<FinancialSummary>,
) -> Result<usize, AppError> {
    if items.is_empty() {
        return Ok(0);
    }

    let count = items.len();
    let active_models = items
        .into_iter()
        .map(|summary| financial_summary::ActiveModel {
            code: Set(summary.code),
            disclosure_no: Set(summary.disclosure_no),
            disclosure_date: Set(summary.disclosure_date),
            report_group_key: Set(summary.report_group_key),
            document_type: Set(summary.document_type),
            current_period_type: Set(summary.current_period_type),
            current_period_start: Set(summary.current_period_start),
            current_period_end: Set(summary.current_period_end),
            current_fiscal_year_start: Set(summary.current_fiscal_year_start),
            current_fiscal_year_end: Set(summary.current_fiscal_year_end),
            sales: Set(summary.sales),
            operating_profit: Set(summary.operating_profit),
            ordinary_profit: Set(summary.ordinary_profit),
            net_profit: Set(summary.net_profit),
            eps: Set(summary.eps),
            bps: Set(summary.bps),
            total_assets: Set(summary.total_assets),
            equity: Set(summary.equity),
            equity_to_asset_ratio: Set(summary.equity_to_asset_ratio),
            roe: Set(summary.roe),
            cash_flow_operating: Set(summary.cash_flow_operating),
            cash_flow_investing: Set(summary.cash_flow_investing),
            cash_flow_financing: Set(summary.cash_flow_financing),
            cash_and_equivalents: Set(summary.cash_and_equivalents),
            dividend_annual: Set(summary.dividend_annual),
            dividend_annual_forecast: Set(summary.dividend_annual_forecast),
            dividend_annual_forecast_next: Set(summary.dividend_annual_forecast_next),
            forecast_sales: Set(summary.forecast_sales),
            forecast_operating_profit: Set(summary.forecast_operating_profit),
            forecast_ordinary_profit: Set(summary.forecast_ordinary_profit),
            forecast_net_profit: Set(summary.forecast_net_profit),
            forecast_eps: Set(summary.forecast_eps),
            next_forecast_sales: Set(summary.next_forecast_sales),
            next_forecast_operating_profit: Set(summary.next_forecast_operating_profit),
            next_forecast_ordinary_profit: Set(summary.next_forecast_ordinary_profit),
            next_forecast_net_profit: Set(summary.next_forecast_net_profit),
            next_forecast_eps: Set(summary.next_forecast_eps),
        })
        .collect::<Vec<_>>();

    let columns_per_row = financial_summary::Column::iter().count();
    let chunk_size = (u16::MAX as usize / columns_per_row).min(MAX_UPSERT_ROWS_PER_STATEMENT);
    let transaction = db.begin().await?;

    for chunk in active_models.chunks(chunk_size) {
        financial_summary::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([
                    financial_summary::Column::Code,
                    financial_summary::Column::DisclosureNo,
                ])
                .update_columns(financial_summary::Column::iter().filter(|column| {
                    !matches!(
                        column,
                        financial_summary::Column::Code | financial_summary::Column::DisclosureNo
                    )
                }))
                .to_owned(),
            )
            .exec_without_returning(&transaction)
            .await?;
    }

    transaction.commit().await?;

    Ok(count)
}

/// 財務情報を取り込む 1 サイクル。取得元が取得可能範囲を返さない場合はスキップする。
pub async fn run_ingest_cycle(
    db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    source: &dyn FinancialSummarySource,
    today: NaiveDate,
) -> Result<IngestStats, AppError> {
    let Some(range) = source.fetchable_range(today) else {
        tracing::debug!("財務情報を取得できないため取り込みをスキップします");
        return Ok(IngestStats::default());
    };

    let latest_stored = find_latest_disc_date(db).await?;
    let Some((from, to)) = fetch_range(range, latest_stored) else {
        tracing::debug!("財務情報の取得可能範囲がデータ提供開始日に届かないためスキップします");
        return Ok(IngestStats::default());
    };

    let mut stats = IngestStats::default();
    let mut date = from;
    while date <= to {
        if crate::date_utils::latest_business_day(date) == date {
            stats.days_attempted += 1;
            match source.fetch_financial_summaries_by_date(date).await {
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
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedFinancialSummarySource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_ingest_cycle(&db, source.as_ref(), Utc::now().date_naive()).await {
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
    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::entities::financial_summary;
    use crate::models::jquants_plan::JQuantsPlan;
    use rstest::rstest;
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use serde_json::json;
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
        DateRange { from: date(2016, 9, 15), to: date(2026, 9, 13) },
        None,
        (date(2016, 9, 15), date(2026, 9, 13))
    )]
    #[case::resumes_with_lookback(
        DateRange { from: date(2016, 9, 15), to: date(2026, 9, 13) },
        Some(date(2026, 8, 1)),
        (date(2026, 7, 2), date(2026, 9, 13))
    )]
    #[case::lookback_clamped_to_plan_from(
        DateRange { from: date(2016, 9, 15), to: date(2026, 9, 13) },
        Some(date(2016, 9, 20)),
        (date(2016, 9, 15), date(2026, 9, 13))
    )]
    #[case::provision_start_date_clamps_range(
        DateRange { from: date(2000, 1, 1), to: date(2026, 9, 13) },
        None,
        (date(2008, 7, 7), date(2026, 9, 13))
    )]
    fn test_fetch_range(
        #[case] range: DateRange,
        #[case] latest_stored: Option<NaiveDate>,
        #[case] expected: (NaiveDate, NaiveDate),
    ) {
        assert_eq!(fetch_range(range, latest_stored), Some(expected));
    }

    #[backend_test_macros::database_test]
    async fn test_ingests_and_upserts_new_disclosures(db: crate::database::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");

        let today = Utc::now().date_naive();
        let to = crate::date_utils::latest_business_day(today);
        // 格納済み最新日を直近にしておき、ルックバック期間と数日分に絞る
        let seed_disc_date = to - chrono::Duration::days(3);
        seed_fin_summary(&db, "00000", "0", seed_disc_date).await;

        mock.fin_summary()
            .date(&to.format("%Y-%m-%d").to_string())
            .items(vec![json!({
                "DiscDate": to.format("%Y-%m-%d").to_string(),
                "Code": "99990",
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

        let range = client.fetchable_range(today).expect("fetchable range");
        let (range_from, range_to) = fetch_range(range, Some(seed_disc_date)).expect("range");
        let stats = run_ingest_cycle(&db, &client, today)
            .await
            .expect("cycle ok");
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: count_business_days(range_from, range_to),
                upserted: 1,
            }
        );

        let row = financial_summary::Entity::find_by_id(("99990".to_string(), "1".to_string()))
            .one(&db)
            .await
            .expect("query ok")
            .expect("row exists");
        assert_eq!(
            row,
            financial_summary::Model {
                code: "99990".to_string(),
                disclosure_no: "1".to_string(),
                disclosure_date: to,
                report_group_key: "N;N;N;".to_string(),
                document_type: None,
                current_period_type: None,
                current_period_start: None,
                current_period_end: None,
                current_fiscal_year_start: None,
                current_fiscal_year_end: None,
                sales: Some(1_000_000.0),
                operating_profit: None,
                ordinary_profit: None,
                net_profit: None,
                eps: None,
                bps: None,
                total_assets: None,
                equity: None,
                equity_to_asset_ratio: None,
                roe: None,
                cash_flow_operating: None,
                cash_flow_investing: None,
                cash_flow_financing: None,
                cash_and_equivalents: None,
                dividend_annual: None,
                dividend_annual_forecast: None,
                dividend_annual_forecast_next: None,
                forecast_sales: None,
                forecast_operating_profit: None,
                forecast_ordinary_profit: None,
                forecast_net_profit: None,
                forecast_eps: None,
                next_forecast_sales: None,
                next_forecast_operating_profit: None,
                next_forecast_ordinary_profit: None,
                next_forecast_net_profit: None,
                next_forecast_eps: None,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn test_replaces_all_fields_for_corrected_disclosures(
        db: crate::database::DatabaseHandle,
    ) {
        let mock = JQuantsMockServer::start().await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");

        let today = Utc::now().date_naive();
        let to = crate::date_utils::latest_business_day(today);
        let seed_disc_date = to - chrono::Duration::days(1);
        seed_populated_fin_summary(&db, seed_disc_date).await;

        mock.fin_summary()
            .date(&to.format("%Y-%m-%d").to_string())
            .items(vec![json!({
                "DiscDate": to.format("%Y-%m-%d").to_string(),
                "Code": "99990",
                "DiscNo": "1",
                "DocType": "QuarterlyStatement",
                "CurPerType": "1Q",
                "CurPerSt": "2026-04-01",
                "CurPerEn": "2026-06-30",
                "CurFYSt": "2026-01-01",
                "CurFYEn": "2026-12-31",
                "Sales": "201",
                "OP": "202",
                "OdP": "203",
                "NP": "204",
                "EPS": "205",
                "BPS": "206",
                "TA": "207",
                "Eq": "208",
                "EqAR": "209",
                "ROE": "210",
                "CFO": "211",
                "CFI": "212",
                "CFF": "213",
                "CashEq": "214",
                "DivAnn": "215",
                "FDivAnn": "216",
                "NxFDivAnn": "217",
                "FSales": "218",
                "FOP": "219",
                "FOdP": "220",
                "FNP": "221",
                "FEPS": "222",
                "NxFSales": "223",
                "NxFOP": "224",
                "NxFOdP": "225",
                "NxFNp": "226",
                "NxFEPS": "227",
            })])
            .ok()
            .await;

        run_ingest_cycle(&db, &client, today)
            .await
            .expect("cycle succeeds");

        let actual = financial_summary::Entity::find_by_id(("99990".to_string(), "1".to_string()))
            .one(&db)
            .await
            .expect("query succeeds")
            .expect("corrected row exists");
        assert_eq!(
            actual,
            financial_summary::Model {
                code: "99990".to_string(),
                disclosure_no: "1".to_string(),
                disclosure_date: to,
                report_group_key: "V18:QuarterlyStatement;V10:2026-04-01;V10:2026-06-30;"
                    .to_string(),
                document_type: Some("QuarterlyStatement".to_string()),
                current_period_type: Some("1Q".to_string()),
                current_period_start: Some(date(2026, 4, 1)),
                current_period_end: Some(date(2026, 6, 30)),
                current_fiscal_year_start: Some(date(2026, 1, 1)),
                current_fiscal_year_end: Some(date(2026, 12, 31)),
                sales: Some(201.0),
                operating_profit: Some(202.0),
                ordinary_profit: Some(203.0),
                net_profit: Some(204.0),
                eps: Some(205.0),
                bps: Some(206.0),
                total_assets: Some(207.0),
                equity: Some(208.0),
                equity_to_asset_ratio: Some(209.0),
                roe: Some(210.0),
                cash_flow_operating: Some(211.0),
                cash_flow_investing: Some(212.0),
                cash_flow_financing: Some(213.0),
                cash_and_equivalents: Some(214.0),
                dividend_annual: Some(215.0),
                dividend_annual_forecast: Some(216.0),
                dividend_annual_forecast_next: Some(217.0),
                forecast_sales: Some(218.0),
                forecast_operating_profit: Some(219.0),
                forecast_ordinary_profit: Some(220.0),
                forecast_net_profit: Some(221.0),
                forecast_eps: Some(222.0),
                next_forecast_sales: Some(223.0),
                next_forecast_operating_profit: Some(224.0),
                next_forecast_ordinary_profit: Some(225.0),
                next_forecast_net_profit: Some(226.0),
                next_forecast_eps: Some(227.0),
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn test_continues_past_days_that_fail_to_fetch(db: crate::database::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");

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
                "Code": "99990",
                "DiscNo": "1",
            })])
            .ok()
            .await;

        let range = client.fetchable_range(today).expect("fetchable range");
        let (range_from, range_to) = fetch_range(range, Some(seed_disc_date)).expect("range");
        let stats = run_ingest_cycle(&db, &client, today)
            .await
            .expect("cycle ok");
        assert_eq!(
            stats,
            IngestStats {
                days_attempted: count_business_days(range_from, range_to),
                upserted: 1,
            }
        );
    }

    async fn seed_fin_summary(
        db: &impl ConnectionTrait,
        code: &str,
        disc_no: &str,
        disc_date: NaiveDate,
    ) {
        financial_summary::ActiveModel {
            code: Set(code.to_string()),
            disclosure_no: Set(disc_no.to_string()),
            disclosure_date: Set(disc_date),
            report_group_key: Set("N;N;N;".to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("seed fin summary");
    }

    async fn seed_populated_fin_summary(db: &impl ConnectionTrait, disc_date: NaiveDate) {
        financial_summary::ActiveModel {
            code: Set("99990".to_string()),
            disclosure_no: Set("1".to_string()),
            disclosure_date: Set(disc_date),
            report_group_key: Set("old report group".to_string()),
            document_type: Set(Some("OldStatement".to_string())),
            current_period_type: Set(Some("FY".to_string())),
            current_period_start: Set(Some(date(2025, 1, 1))),
            current_period_end: Set(Some(date(2025, 12, 31))),
            current_fiscal_year_start: Set(Some(date(2025, 1, 1))),
            current_fiscal_year_end: Set(Some(date(2025, 12, 31))),
            sales: Set(Some(1.0)),
            operating_profit: Set(Some(1.0)),
            ordinary_profit: Set(Some(1.0)),
            net_profit: Set(Some(1.0)),
            eps: Set(Some(1.0)),
            bps: Set(Some(1.0)),
            total_assets: Set(Some(1.0)),
            equity: Set(Some(1.0)),
            equity_to_asset_ratio: Set(Some(1.0)),
            roe: Set(Some(1.0)),
            cash_flow_operating: Set(Some(1.0)),
            cash_flow_investing: Set(Some(1.0)),
            cash_flow_financing: Set(Some(1.0)),
            cash_and_equivalents: Set(Some(1.0)),
            dividend_annual: Set(Some(1.0)),
            dividend_annual_forecast: Set(Some(1.0)),
            dividend_annual_forecast_next: Set(Some(1.0)),
            forecast_sales: Set(Some(1.0)),
            forecast_operating_profit: Set(Some(1.0)),
            forecast_ordinary_profit: Set(Some(1.0)),
            forecast_net_profit: Set(Some(1.0)),
            forecast_eps: Set(Some(1.0)),
            next_forecast_sales: Set(Some(1.0)),
            next_forecast_operating_profit: Set(Some(1.0)),
            next_forecast_ordinary_profit: Set(Some(1.0)),
            next_forecast_net_profit: Set(Some(1.0)),
            next_forecast_eps: Set(Some(1.0)),
        }
        .insert(db)
        .await
        .expect("seed populated financial summary");
    }
}
