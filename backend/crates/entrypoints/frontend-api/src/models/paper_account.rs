use chrono::NaiveDate;
use core_application::paper_trade::{NewPaperAccount, PaperAccount};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = PaperAccount)]
pub struct PaperAccountResponse {
    pub id: Uuid,
    pub name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub initial_cash_jpy: Decimal,
    pub benchmark_stock_id: Option<String>,
    pub started_on: NaiveDate,
}

impl From<PaperAccount> for PaperAccountResponse {
    fn from(account: PaperAccount) -> Self {
        Self {
            id: account.id,
            name: account.name,
            strategy_id: account.strategy_id,
            purpose: account.purpose,
            initial_cash_jpy: account.initial_cash_jpy,
            benchmark_stock_id: account.benchmark_stock_id,
            started_on: account.started_on,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePaperAccountRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub strategy_id: Uuid,
    #[schema(min_length = 1, pattern = r"\S")]
    pub purpose: String,
    pub initial_cash_jpy: Decimal,
    #[serde(default)]
    pub benchmark_stock_id: Option<String>,
    pub started_on: NaiveDate,
}

impl From<CreatePaperAccountRequest> for NewPaperAccount {
    fn from(request: CreatePaperAccountRequest) -> Self {
        Self {
            name: request.name,
            strategy_id: request.strategy_id,
            purpose: request.purpose,
            initial_cash_jpy: request.initial_cash_jpy,
            benchmark_stock_id: request.benchmark_stock_id,
            started_on: request.started_on,
        }
    }
}
