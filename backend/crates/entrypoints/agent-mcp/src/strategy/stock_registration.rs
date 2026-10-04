use core_application::stock_registration::{RegisterStockCommand, StockRegistrationUseCaseError};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, internal_error, invalid_params};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RegisterStockParams {
    pub country: String,
    pub code: String,
    pub name: String,
    pub exchange: String,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct RegisterStockResult {
    pub id: String,
    pub name: String,
    pub exchange: String,
}

impl StrategyServer {
    pub(crate) async fn register_stock_inner(
        &self,
        params: RegisterStockParams,
    ) -> Result<RegisterStockResult, McpError> {
        let registered = self
            .dependencies
            .stock_registration
            .register(RegisterStockCommand {
                country: params.country,
                code: params.code,
                name: params.name,
                exchange: params.exchange,
            })
            .await
            .map_err(stock_registration_error)?;

        Ok(RegisterStockResult {
            id: registered.id,
            name: registered.name,
            exchange: registered.exchange,
        })
    }
}

fn stock_registration_error(error: StockRegistrationUseCaseError) -> McpError {
    match error {
        StockRegistrationUseCaseError::Validation(message) => invalid_params(message),
        other => {
            tracing::error!(error = %other, "strategy mcp stock registration failed");
            internal_error(format!("stock registration failed: {other}"))
        }
    }
}
