use chrono::{DateTime, NaiveDate, Utc};
use core_application::stock_group::StockGroupMembership;
use core_domain::calendar_event::CalendarEvent;
use core_domain::financial_summary::FinancialSummary;
use core_domain::valuation::Valuation;
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockGroupKeyAndNameResponse {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockGroupMembershipResponse {
    pub axis: StockGroupKeyAndNameResponse,
    pub group: StockGroupKeyAndNameResponse,
}

impl From<StockGroupMembership> for StockGroupMembershipResponse {
    fn from(membership: StockGroupMembership) -> Self {
        Self {
            axis: StockGroupKeyAndNameResponse {
                key: membership.axis_key,
                name: membership.axis_name,
            },
            group: StockGroupKeyAndNameResponse {
                key: membership.group_key,
                name: membership.group_name,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockNextEarningsResponse {
    pub title: String,
    pub fiscal_period: Option<String>,
    pub event_date: NaiveDate,
    pub event_at: Option<DateTime<Utc>>,
    pub time_of_day: Option<String>,
}

impl From<CalendarEvent> for StockNextEarningsResponse {
    fn from(event: CalendarEvent) -> Self {
        Self {
            title: event.title,
            fiscal_period: event.fiscal_period,
            event_date: event.event_date,
            event_at: event.event_at,
            time_of_day: event.time_of_day.map(|value| value.as_str().to_owned()),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockValuationResponse {
    pub code: String,
    pub date: NaiveDate,
    pub eps: Option<Decimal>,
    pub fwd_eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub fwd_roe: Option<Decimal>,
    pub per: Option<Decimal>,
    pub fwd_per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub mkt_cap: Option<Decimal>,
}

impl From<Valuation> for StockValuationResponse {
    fn from(valuation: Valuation) -> Self {
        Self {
            code: valuation.code,
            date: valuation.date,
            eps: valuation.eps,
            fwd_eps: valuation.fwd_eps,
            bps: valuation.bps,
            roe: valuation.roe,
            fwd_roe: valuation.fwd_roe,
            per: valuation.per,
            fwd_per: valuation.fwd_per,
            pbr: valuation.pbr,
            mkt_cap: valuation.mkt_cap,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockFinancialSummaryResponse {
    pub code: String,
    pub disclosure_no: String,
    pub disclosure_date: NaiveDate,
    pub report_group_key: String,
    pub document_type: Option<String>,
    pub current_period_type: Option<String>,
    pub current_period_start: Option<NaiveDate>,
    pub current_period_end: Option<NaiveDate>,
    pub current_fiscal_year_start: Option<NaiveDate>,
    pub current_fiscal_year_end: Option<NaiveDate>,
    pub sales: Option<f64>,
    pub operating_profit: Option<f64>,
    pub ordinary_profit: Option<f64>,
    pub net_profit: Option<f64>,
    pub eps: Option<f64>,
    pub bps: Option<f64>,
    pub total_assets: Option<f64>,
    pub equity: Option<f64>,
    pub equity_to_asset_ratio: Option<f64>,
    pub roe: Option<f64>,
    pub cash_flow_operating: Option<f64>,
    pub cash_flow_investing: Option<f64>,
    pub cash_flow_financing: Option<f64>,
    pub cash_and_equivalents: Option<f64>,
    pub dividend_annual: Option<f64>,
    pub dividend_annual_forecast: Option<f64>,
    pub dividend_annual_forecast_next: Option<f64>,
    pub forecast_sales: Option<f64>,
    pub forecast_operating_profit: Option<f64>,
    pub forecast_ordinary_profit: Option<f64>,
    pub forecast_net_profit: Option<f64>,
    pub forecast_eps: Option<f64>,
    pub next_forecast_sales: Option<f64>,
    pub next_forecast_operating_profit: Option<f64>,
    pub next_forecast_ordinary_profit: Option<f64>,
    pub next_forecast_net_profit: Option<f64>,
    pub next_forecast_eps: Option<f64>,
}

impl From<FinancialSummary> for StockFinancialSummaryResponse {
    fn from(summary: FinancialSummary) -> Self {
        Self {
            code: summary.code,
            disclosure_no: summary.disclosure_no,
            disclosure_date: summary.disclosure_date,
            report_group_key: summary.report_group_key,
            document_type: summary.document_type,
            current_period_type: summary.current_period_type,
            current_period_start: summary.current_period_start,
            current_period_end: summary.current_period_end,
            current_fiscal_year_start: summary.current_fiscal_year_start,
            current_fiscal_year_end: summary.current_fiscal_year_end,
            sales: summary.sales,
            operating_profit: summary.operating_profit,
            ordinary_profit: summary.ordinary_profit,
            net_profit: summary.net_profit,
            eps: summary.eps,
            bps: summary.bps,
            total_assets: summary.total_assets,
            equity: summary.equity,
            equity_to_asset_ratio: summary.equity_to_asset_ratio,
            roe: summary.roe,
            cash_flow_operating: summary.cash_flow_operating,
            cash_flow_investing: summary.cash_flow_investing,
            cash_flow_financing: summary.cash_flow_financing,
            cash_and_equivalents: summary.cash_and_equivalents,
            dividend_annual: summary.dividend_annual,
            dividend_annual_forecast: summary.dividend_annual_forecast,
            dividend_annual_forecast_next: summary.dividend_annual_forecast_next,
            forecast_sales: summary.forecast_sales,
            forecast_operating_profit: summary.forecast_operating_profit,
            forecast_ordinary_profit: summary.forecast_ordinary_profit,
            forecast_net_profit: summary.forecast_net_profit,
            forecast_eps: summary.forecast_eps,
            next_forecast_sales: summary.next_forecast_sales,
            next_forecast_operating_profit: summary.next_forecast_operating_profit,
            next_forecast_ordinary_profit: summary.next_forecast_ordinary_profit,
            next_forecast_net_profit: summary.next_forecast_net_profit,
            next_forecast_eps: summary.next_forecast_eps,
        }
    }
}
