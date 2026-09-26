use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_122810_migrate_financial_summaries_to_neutral_table"
    }
}

#[derive(DeriveIden)]
enum FinancialSummary {
    Table,
    Code,
    DisclosureNo,
    DisclosureDate,
    ReportGroupKey,
    DocumentType,
    CurrentPeriodType,
    CurrentPeriodStart,
    CurrentPeriodEnd,
    CurrentFiscalYearStart,
    CurrentFiscalYearEnd,
    Sales,
    OperatingProfit,
    OrdinaryProfit,
    NetProfit,
    Eps,
    Bps,
    TotalAssets,
    Equity,
    EquityToAssetRatio,
    Roe,
    CashFlowOperating,
    CashFlowInvesting,
    CashFlowFinancing,
    CashAndEquivalents,
    DividendAnnual,
    DividendAnnualForecast,
    DividendAnnualForecastNext,
    ForecastSales,
    ForecastOperatingProfit,
    ForecastOrdinaryProfit,
    ForecastNetProfit,
    ForecastEps,
    NextForecastSales,
    NextForecastOperatingProfit,
    NextForecastOrdinaryProfit,
    NextForecastNetProfit,
    NextForecastEps,
}

#[derive(DeriveIden)]
enum JQuantsFinSummary {
    #[sea_orm(iden = "jquants_fin_summary")]
    Table,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FinancialSummary::Table)
                    .col(ColumnDef::new(FinancialSummary::Code).string().not_null())
                    .col(
                        ColumnDef::new(FinancialSummary::DisclosureNo)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FinancialSummary::DisclosureDate)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FinancialSummary::ReportGroupKey)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FinancialSummary::DocumentType).string())
                    .col(ColumnDef::new(FinancialSummary::CurrentPeriodType).string())
                    .col(ColumnDef::new(FinancialSummary::CurrentPeriodStart).date())
                    .col(ColumnDef::new(FinancialSummary::CurrentPeriodEnd).date())
                    .col(ColumnDef::new(FinancialSummary::CurrentFiscalYearStart).date())
                    .col(ColumnDef::new(FinancialSummary::CurrentFiscalYearEnd).date())
                    .col(ColumnDef::new(FinancialSummary::Sales).double())
                    .col(ColumnDef::new(FinancialSummary::OperatingProfit).double())
                    .col(ColumnDef::new(FinancialSummary::OrdinaryProfit).double())
                    .col(ColumnDef::new(FinancialSummary::NetProfit).double())
                    .col(ColumnDef::new(FinancialSummary::Eps).double())
                    .col(ColumnDef::new(FinancialSummary::Bps).double())
                    .col(ColumnDef::new(FinancialSummary::TotalAssets).double())
                    .col(ColumnDef::new(FinancialSummary::Equity).double())
                    .col(ColumnDef::new(FinancialSummary::EquityToAssetRatio).double())
                    .col(ColumnDef::new(FinancialSummary::Roe).double())
                    .col(ColumnDef::new(FinancialSummary::CashFlowOperating).double())
                    .col(ColumnDef::new(FinancialSummary::CashFlowInvesting).double())
                    .col(ColumnDef::new(FinancialSummary::CashFlowFinancing).double())
                    .col(ColumnDef::new(FinancialSummary::CashAndEquivalents).double())
                    .col(ColumnDef::new(FinancialSummary::DividendAnnual).double())
                    .col(ColumnDef::new(FinancialSummary::DividendAnnualForecast).double())
                    .col(ColumnDef::new(FinancialSummary::DividendAnnualForecastNext).double())
                    .col(ColumnDef::new(FinancialSummary::ForecastSales).double())
                    .col(ColumnDef::new(FinancialSummary::ForecastOperatingProfit).double())
                    .col(ColumnDef::new(FinancialSummary::ForecastOrdinaryProfit).double())
                    .col(ColumnDef::new(FinancialSummary::ForecastNetProfit).double())
                    .col(ColumnDef::new(FinancialSummary::ForecastEps).double())
                    .col(ColumnDef::new(FinancialSummary::NextForecastSales).double())
                    .col(ColumnDef::new(FinancialSummary::NextForecastOperatingProfit).double())
                    .col(ColumnDef::new(FinancialSummary::NextForecastOrdinaryProfit).double())
                    .col(ColumnDef::new(FinancialSummary::NextForecastNetProfit).double())
                    .col(ColumnDef::new(FinancialSummary::NextForecastEps).double())
                    .primary_key(
                        Index::create()
                            .col(FinancialSummary::Code)
                            .col(FinancialSummary::DisclosureNo),
                    )
                    .to_owned(),
            )
            .await?;

        // 不正な任意項目は欠損値 (NULL) として扱う。
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE FUNCTION m20260925_fin_summary_try_date(value text)
                RETURNS date LANGUAGE plpgsql IMMUTABLE STRICT AS $$
                BEGIN
                    IF value !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' THEN
                        RETURN NULL;
                    END IF;
                    RETURN value::date;
                EXCEPTION WHEN OTHERS THEN
                    RETURN NULL;
                END;
                $$",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE FUNCTION m20260925_fin_summary_try_double(value text)
                RETURNS double precision LANGUAGE plpgsql IMMUTABLE STRICT AS $$
                BEGIN
                    RETURN value::double precision;
                EXCEPTION WHEN OTHERS THEN
                    RETURN NULL;
                END;
                $$",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE FUNCTION m20260925_fin_summary_report_group_key(summary jsonb)
                RETURNS text LANGUAGE sql IMMUTABLE AS $$
                    SELECT
                        CASE WHEN summary->>'DocType' IS NULL THEN 'N;'
                            ELSE 'V' || octet_length(summary->>'DocType')::text
                                || ':' || (summary->>'DocType') || ';' END ||
                        CASE WHEN summary->>'CurPerSt' IS NULL THEN 'N;'
                            ELSE 'V' || octet_length(summary->>'CurPerSt')::text
                                || ':' || (summary->>'CurPerSt') || ';' END ||
                        CASE WHEN summary->>'CurPerEn' IS NULL THEN 'N;'
                            ELSE 'V' || octet_length(summary->>'CurPerEn')::text
                                || ':' || (summary->>'CurPerEn') || ';' END
                $$",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO financial_summary (
                    code, disclosure_no, disclosure_date, report_group_key, document_type,
                    current_period_type,
                    current_period_start, current_period_end, current_fiscal_year_start,
                    current_fiscal_year_end, sales, operating_profit, ordinary_profit, net_profit,
                    eps, bps, total_assets, equity, equity_to_asset_ratio, roe,
                    cash_flow_operating, cash_flow_investing, cash_flow_financing,
                    cash_and_equivalents, dividend_annual, dividend_annual_forecast,
                    dividend_annual_forecast_next, forecast_sales, forecast_operating_profit,
                    forecast_ordinary_profit, forecast_net_profit, forecast_eps,
                    next_forecast_sales, next_forecast_operating_profit,
                    next_forecast_ordinary_profit, next_forecast_net_profit, next_forecast_eps
                )
                SELECT code, disc_no, disc_date,
                    m20260925_fin_summary_report_group_key(raw),
                    NULLIF(raw->>'DocType', ''), NULLIF(raw->>'CurPerType', ''),
                    m20260925_fin_summary_try_date(NULLIF(raw->>'CurPerSt', '')),
                    m20260925_fin_summary_try_date(NULLIF(raw->>'CurPerEn', '')),
                    m20260925_fin_summary_try_date(NULLIF(raw->>'CurFYSt', '')),
                    m20260925_fin_summary_try_date(NULLIF(raw->>'CurFYEn', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'Sales', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'OP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'OdP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'EPS', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'BPS', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'TA', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'Eq', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'EqAR', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'ROE', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'CFO', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'CFI', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'CFF', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'CashEq', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'DivAnn', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'FDivAnn', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NxFDivAnn', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'FSales', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'FOP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'FOdP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'FNP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'FEPS', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NxFSales', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NxFOP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NxFOdP', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NxFNp', '')),
                    m20260925_fin_summary_try_double(NULLIF(raw->>'NxFEPS', ''))
                FROM jquants_fin_summary",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION m20260925_fin_summary_try_date(text)")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION m20260925_fin_summary_try_double(text)")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION m20260925_fin_summary_report_group_key(jsonb)")
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_financial_summary_disclosure_date")
                    .table(FinancialSummary::Table)
                    .col(FinancialSummary::DisclosureDate)
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_financial_summary_code_prefix \
                 ON financial_summary (LEFT(code, 4))",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(JQuantsFinSummary::Table).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(JQuantsFinSummary::Table)
                    .col(ColumnDef::new(Alias::new("code")).string().not_null())
                    .col(ColumnDef::new(Alias::new("disc_no")).string().not_null())
                    .col(ColumnDef::new(Alias::new("disc_date")).date().not_null())
                    .col(ColumnDef::new(Alias::new("raw")).json_binary().not_null())
                    .primary_key(
                        Index::create()
                            .col(Alias::new("code"))
                            .col(Alias::new("disc_no")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO jquants_fin_summary (code, disc_no, disc_date, raw)
                SELECT code, disclosure_no, disclosure_date, jsonb_strip_nulls(jsonb_build_object(
                    'Code', code, 'DiscNo', disclosure_no,
                    'DiscDate', to_char(disclosure_date, 'YYYY-MM-DD'),
                    'DocType', document_type, 'CurPerType', current_period_type,
                    'CurPerSt', to_char(current_period_start, 'YYYY-MM-DD'),
                    'CurPerEn', to_char(current_period_end, 'YYYY-MM-DD'),
                    'CurFYSt', to_char(current_fiscal_year_start, 'YYYY-MM-DD'),
                    'CurFYEn', to_char(current_fiscal_year_end, 'YYYY-MM-DD'),
                    'Sales', sales::text, 'OP', operating_profit::text,
                    'OdP', ordinary_profit::text, 'NP', net_profit::text,
                    'EPS', eps::text, 'BPS', bps::text, 'TA', total_assets::text,
                    'Eq', equity::text, 'EqAR', equity_to_asset_ratio::text,
                    'ROE', roe::text, 'CFO', cash_flow_operating::text,
                    'CFI', cash_flow_investing::text, 'CFF', cash_flow_financing::text,
                    'CashEq', cash_and_equivalents::text, 'DivAnn', dividend_annual::text,
                    'FDivAnn', dividend_annual_forecast::text,
                    'NxFDivAnn', dividend_annual_forecast_next::text,
                    'FSales', forecast_sales::text, 'FOP', forecast_operating_profit::text,
                    'FOdP', forecast_ordinary_profit::text, 'FNP', forecast_net_profit::text,
                    'FEPS', forecast_eps::text, 'NxFSales', next_forecast_sales::text,
                    'NxFOP', next_forecast_operating_profit::text,
                    'NxFOdP', next_forecast_ordinary_profit::text,
                    'NxFNp', next_forecast_net_profit::text, 'NxFEPS', next_forecast_eps::text
                ))
                FROM financial_summary",
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_jquants_fin_summary_disc_date")
                    .table(JQuantsFinSummary::Table)
                    .col(Alias::new("disc_date"))
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_jquants_fin_summary_code_prefix \
                 ON jquants_fin_summary (LEFT(code, 4))",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(FinancialSummary::Table).to_owned())
            .await
    }
}
