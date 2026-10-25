use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::financial_summary::{
    FinancialSummaryRepository, FinancialSummaryRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::financial_summary::{FinancialSummary, FinancialSummaryPeriodType};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, Iterable, QueryOrder, Set,
    Statement,
};

use crate::DatabaseHandle;
use crate::entities::financial_summary;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const MAX_UPSERT_ROWS_PER_STATEMENT: usize = 1_000;

const FIND_FOR_SYMBOL_SQL: &str = r#"
    -- 同じ書類種別・会計期間の開示が複数あれば開示番号が最大の 1 件のみ残す
    -- (訂正、あるいは業績予想修正の再修正)
    WITH deduped AS (
        SELECT DISTINCT ON (report_group_key) financial_summary.*,
            disclosure_no::bigint AS disclosure_no_num
        FROM financial_summary
        WHERE LEFT(code, 4) = $1
        ORDER BY report_group_key, disclosure_no_num DESC
    )
    SELECT *
    FROM deduped
    ORDER BY disclosure_date DESC, disclosure_no_num DESC
    LIMIT $2
"#;

const FIND_FOR_PERIOD_SQL: &str = r#"
    SELECT *
    FROM financial_summary
    WHERE (code = $1 OR (char_length($1) = 4 AND code = $1 || '0'))
      AND current_period_end = $2
      AND current_period_type = $3
    ORDER BY disclosure_date DESC, document_type ASC, report_group_key ASC
"#;

#[derive(Clone)]
pub struct PostgresFinancialSummaryRepository {
    db: DatabaseHandle,
}

impl PostgresFinancialSummaryRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FinancialSummaryRepository for PostgresFinancialSummaryRepository {
    async fn latest_disclosure_date(
        &self,
    ) -> Result<Option<NaiveDate>, FinancialSummaryRepositoryError> {
        financial_summary::Entity::find()
            .order_by_desc(financial_summary::Column::DisclosureDate)
            .one(&self.db)
            .await
            .map(|row| row.map(|row| row.disclosure_date))
            .map_err(repository_error)
    }

    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        summaries: Vec<FinancialSummary>,
    ) -> Result<usize, FinancialSummaryRepositoryError> {
        if summaries.is_empty() {
            return Ok(0);
        }

        let transaction = transaction_ref(transaction)?;
        let count = summaries.len();
        let active_models = summaries
            .into_iter()
            .map(to_active_model)
            .collect::<Vec<_>>();
        let columns_per_row = financial_summary::Column::iter().count();
        let chunk_size = crate::repositories::max_rows_per_statement(columns_per_row)
            .min(MAX_UPSERT_ROWS_PER_STATEMENT);

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
                            financial_summary::Column::Code
                                | financial_summary::Column::DisclosureNo
                        )
                    }))
                    .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        Ok(count)
    }

    async fn find_for_symbol(
        &self,
        symbol: &str,
        limit: u64,
    ) -> Result<Vec<FinancialSummary>, FinancialSummaryRepositoryError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                FIND_FOR_SYMBOL_SQL,
                [symbol.into(), limit.into()],
            ))
            .await
            .map_err(repository_error)?;

        rows.iter()
            .map(|row| financial_summary::Model::from_query_result(row, ""))
            .map(|row| row.map(to_domain))
            .collect::<Result<Vec<_>, _>>()
            .map_err(repository_error)
    }

    async fn find_for_period(
        &self,
        symbol: &str,
        period_end: NaiveDate,
        period_type: FinancialSummaryPeriodType,
    ) -> Result<Vec<FinancialSummary>, FinancialSummaryRepositoryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                FIND_FOR_PERIOD_SQL,
                [
                    symbol.into(),
                    period_end.into(),
                    period_type.as_str().into(),
                ],
            ))
            .await
            .map_err(repository_error)?;

        rows.iter()
            .map(|row| financial_summary::Model::from_query_result(row, ""))
            .map(|row| row.map(to_domain))
            .collect::<Result<Vec<_>, _>>()
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, FinancialSummaryRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(FinancialSummaryRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> FinancialSummaryRepositoryError {
    FinancialSummaryRepositoryError::Database(persistence_error(error))
}

fn to_active_model(summary: FinancialSummary) -> financial_summary::ActiveModel {
    financial_summary::ActiveModel {
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
    }
}

fn to_domain(model: financial_summary::Model) -> FinancialSummary {
    FinancialSummary {
        code: model.code,
        disclosure_no: model.disclosure_no,
        disclosure_date: model.disclosure_date,
        report_group_key: model.report_group_key,
        document_type: model.document_type,
        current_period_type: model.current_period_type,
        current_period_start: model.current_period_start,
        current_period_end: model.current_period_end,
        current_fiscal_year_start: model.current_fiscal_year_start,
        current_fiscal_year_end: model.current_fiscal_year_end,
        sales: model.sales,
        operating_profit: model.operating_profit,
        ordinary_profit: model.ordinary_profit,
        net_profit: model.net_profit,
        eps: model.eps,
        bps: model.bps,
        total_assets: model.total_assets,
        equity: model.equity,
        equity_to_asset_ratio: model.equity_to_asset_ratio,
        roe: model.roe,
        cash_flow_operating: model.cash_flow_operating,
        cash_flow_investing: model.cash_flow_investing,
        cash_flow_financing: model.cash_flow_financing,
        cash_and_equivalents: model.cash_and_equivalents,
        dividend_annual: model.dividend_annual,
        dividend_annual_forecast: model.dividend_annual_forecast,
        dividend_annual_forecast_next: model.dividend_annual_forecast_next,
        forecast_sales: model.forecast_sales,
        forecast_operating_profit: model.forecast_operating_profit,
        forecast_ordinary_profit: model.forecast_ordinary_profit,
        forecast_net_profit: model.forecast_net_profit,
        forecast_eps: model.forecast_eps,
        next_forecast_sales: model.next_forecast_sales,
        next_forecast_operating_profit: model.next_forecast_operating_profit,
        next_forecast_ordinary_profit: model.next_forecast_ordinary_profit,
        next_forecast_net_profit: model.next_forecast_net_profit,
        next_forecast_eps: model.next_forecast_eps,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::financial_summary::FinancialSummaryRepository;
    use core_application::unit_of_work::UnitOfWork;

    use super::PostgresFinancialSummaryRepository;
    use crate::unit_of_work::PostgresUnitOfWork;
    use core_domain::financial_summary::{FinancialSummary, FinancialSummaryPeriodType};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn summary(
        code: &str,
        disclosure_no: &str,
        disclosure_date: NaiveDate,
        report_group_key: &str,
        sales: f64,
    ) -> FinancialSummary {
        FinancialSummary {
            code: code.to_owned(),
            disclosure_no: disclosure_no.to_owned(),
            disclosure_date,
            report_group_key: report_group_key.to_owned(),
            document_type: None,
            current_period_type: None,
            current_period_start: None,
            current_period_end: None,
            current_fiscal_year_start: None,
            current_fiscal_year_end: None,
            sales: Some(sales),
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
    }

    fn period_summary(
        code: &str,
        disclosure_no: &str,
        disclosure_date: NaiveDate,
        report_group_key: &str,
        document_type: &str,
        current_period_type: &str,
        current_period_end: NaiveDate,
    ) -> FinancialSummary {
        let mut summary = summary(code, disclosure_no, disclosure_date, report_group_key, 10.0);
        summary.document_type = Some(document_type.to_owned());
        summary.current_period_type = Some(current_period_type.to_owned());
        summary.current_period_end = Some(current_period_end);
        summary
    }

    async fn upsert_summaries(
        repository: &PostgresFinancialSummaryRepository,
        unit_of_work: &PostgresUnitOfWork,
        summaries: Vec<FinancialSummary>,
    ) -> usize {
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let upserted = repository
            .upsert(&transaction, summaries)
            .await
            .expect("upsert summaries");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        upserted
    }

    #[backend_test_macros::database_test]
    async fn upsert_returns_number_of_written_summaries(db: crate::DatabaseHandle) {
        let repository = PostgresFinancialSummaryRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let summary = summary("ZQXZ0", "1", date(2042, 4, 1), "group-a", 10.0);
        let upserted = upsert_summaries(&repository, &unit_of_work, vec![summary]).await;

        assert_eq!(upserted, 1);
    }

    #[backend_test_macros::database_test]
    async fn latest_disclosure_date_returns_the_newest_stored_date(db: crate::DatabaseHandle) {
        let repository = PostgresFinancialSummaryRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        upsert_summaries(
            &repository,
            &unit_of_work,
            vec![
                summary("ZQXZ0", "1", date(2042, 4, 1), "group-a", 10.0),
                summary("ABCD0", "2", date(2042, 4, 5), "group-b", 20.0),
            ],
        )
        .await;
        let latest = repository
            .latest_disclosure_date()
            .await
            .expect("find latest date");

        assert_eq!(latest, Some(date(2042, 4, 5)));
    }

    #[backend_test_macros::database_test]
    async fn find_for_symbol_keeps_highest_disclosure_number_per_group_and_orders_newest_first(
        db: crate::DatabaseHandle,
    ) {
        let repository = PostgresFinancialSummaryRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        upsert_summaries(
            &repository,
            &unit_of_work,
            vec![
                summary("ZQXZ0", "2", date(2042, 4, 3), "group-a", 20.0),
                summary("ZQXZ0", "12", date(2042, 4, 1), "group-a", 120.0),
                summary("ZQXZ0", "1", date(2042, 4, 4), "group-b", 40.0),
            ],
        )
        .await;
        let rows = repository
            .find_for_symbol("ZQXZ", 10)
            .await
            .expect("find summaries");

        assert_eq!(
            rows,
            vec![
                summary("ZQXZ0", "1", date(2042, 4, 4), "group-b", 40.0),
                summary("ZQXZ0", "12", date(2042, 4, 1), "group-a", 120.0),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn find_for_symbol_respects_limit_after_ordering_distinct_groups(
        db: crate::DatabaseHandle,
    ) {
        let repository = PostgresFinancialSummaryRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        upsert_summaries(
            &repository,
            &unit_of_work,
            vec![
                summary("ZQXZ0", "1", date(2042, 4, 1), "group-a", 10.0),
                summary("ZQXZ0", "2", date(2042, 4, 2), "group-b", 20.0),
                summary("ZQXZ0", "3", date(2042, 4, 3), "group-c", 30.0),
            ],
        )
        .await;
        let rows = repository
            .find_for_symbol("ZQXZ", 2)
            .await
            .expect("find summaries");

        assert_eq!(
            rows,
            vec![
                summary("ZQXZ0", "3", date(2042, 4, 3), "group-c", 30.0),
                summary("ZQXZ0", "2", date(2042, 4, 2), "group-b", 20.0),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn find_for_period_keeps_all_exact_period_candidates_without_deduplication(
        db: crate::DatabaseHandle,
    ) {
        let repository = PostgresFinancialSummaryRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let period_end = date(2042, 6, 30);
        upsert_summaries(
            &repository,
            &unit_of_work,
            vec![
                period_summary(
                    "ZQXZ0",
                    "1",
                    date(2042, 8, 1),
                    "sample-group-a",
                    "2QFinancialStatements_SampleA",
                    "2Q",
                    period_end,
                ),
                period_summary(
                    "ZQXZ0",
                    "2",
                    date(2042, 8, 1),
                    "sample-group-a",
                    "2QFinancialStatements_SampleB",
                    "2Q",
                    period_end,
                ),
                period_summary(
                    "ZQXZ0",
                    "3",
                    date(2042, 8, 1),
                    "sample-forecast",
                    "EarnForecastRevision",
                    "2Q",
                    period_end,
                ),
                period_summary(
                    "ZQXZ0",
                    "4",
                    date(2042, 8, 1),
                    "sample-other-quarter",
                    "1QFinancialStatements_SampleC",
                    "1Q",
                    period_end,
                ),
                period_summary(
                    "ZQXZ0",
                    "5",
                    date(2042, 8, 1),
                    "sample-other-period",
                    "2QFinancialStatements_SampleD",
                    "2Q",
                    date(2042, 6, 29),
                ),
                period_summary(
                    "ABCD0",
                    "6",
                    date(2042, 8, 1),
                    "sample-other-stock",
                    "2QFinancialStatements_SampleE",
                    "2Q",
                    period_end,
                ),
                period_summary(
                    "ZQXZ1",
                    "7",
                    date(2042, 8, 1),
                    "sample-prefix-collision",
                    "2QFinancialStatements_SampleF",
                    "2Q",
                    period_end,
                ),
            ],
        )
        .await;

        let rows = repository
            .find_for_period(
                "ZQXZ",
                period_end,
                FinancialSummaryPeriodType::SecondQuarter,
            )
            .await
            .expect("find period candidates");
        let actual = rows
            .into_iter()
            .map(|row| {
                (
                    row.disclosure_no,
                    row.document_type,
                    row.current_period_type,
                )
            })
            .collect::<Vec<_>>();
        let mut actual = actual;
        actual.sort_by(|left, right| left.1.cmp(&right.1));

        assert_eq!(
            actual,
            vec![
                (
                    "1".to_owned(),
                    Some("2QFinancialStatements_SampleA".to_owned()),
                    Some("2Q".to_owned()),
                ),
                (
                    "2".to_owned(),
                    Some("2QFinancialStatements_SampleB".to_owned()),
                    Some("2Q".to_owned()),
                ),
                (
                    "3".to_owned(),
                    Some("EarnForecastRevision".to_owned()),
                    Some("2Q".to_owned()),
                ),
            ],
        );
    }
}
