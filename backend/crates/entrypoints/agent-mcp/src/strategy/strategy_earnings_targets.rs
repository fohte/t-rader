use core_application::strategy_earnings_target::{
    StrategyEarningsTarget, StrategyEarningsTargetUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, internal_error, invalid_params};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EarningsTargetParams {
    pub ref_kind: String,
    pub ref_id: String,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct EarningsTargetDto {
    pub ref_kind: String,
    pub ref_id: String,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct EarningsTargetChangeResult {
    pub changed: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ListEarningsTargetsResult {
    pub targets: Vec<EarningsTargetDto>,
}

impl StrategyServer {
    pub(crate) async fn add_earnings_target_inner(
        &self,
        scope: StrategyScope,
        params: EarningsTargetParams,
    ) -> Result<EarningsTargetChangeResult, McpError> {
        let changed = self
            .dependencies
            .strategy_earnings_targets
            .add(scope, &params.ref_kind, &params.ref_id)
            .await
            .map_err(earnings_target_error)?;
        Ok(EarningsTargetChangeResult { changed })
    }

    pub(crate) async fn remove_earnings_target_inner(
        &self,
        scope: StrategyScope,
        params: EarningsTargetParams,
    ) -> Result<EarningsTargetChangeResult, McpError> {
        let changed = self
            .dependencies
            .strategy_earnings_targets
            .remove(scope, &params.ref_kind, &params.ref_id)
            .await
            .map_err(earnings_target_error)?;
        Ok(EarningsTargetChangeResult { changed })
    }

    pub(crate) async fn list_earnings_targets_inner(
        &self,
        scope: StrategyScope,
    ) -> Result<ListEarningsTargetsResult, McpError> {
        let targets = self
            .dependencies
            .strategy_earnings_targets
            .list(scope)
            .await
            .map_err(earnings_target_error)?;
        Ok(ListEarningsTargetsResult {
            targets: targets.into_iter().map(target_to_dto).collect(),
        })
    }
}

fn target_to_dto(target: StrategyEarningsTarget) -> EarningsTargetDto {
    EarningsTargetDto {
        ref_kind: target.ref_kind,
        ref_id: target.ref_id,
        created_at: target.created_at,
    }
}

fn earnings_target_error(error: StrategyEarningsTargetUseCaseError) -> McpError {
    match error {
        StrategyEarningsTargetUseCaseError::Validation(message)
        | StrategyEarningsTargetUseCaseError::Reference(
            core_application::refs::RefUseCaseError::Validation(message),
        ) => invalid_params(message),
        error @ StrategyEarningsTargetUseCaseError::ReferenceNotFound { .. } => {
            invalid_params(error.to_string())
        }
        error => {
            tracing::error!(error = %error, "strategy mcp earnings target operation failed");
            internal_error(format!("earnings target operation failed: {error}"))
        }
    }
}
