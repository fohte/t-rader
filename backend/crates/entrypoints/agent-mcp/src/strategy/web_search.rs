//! `search_web` tool の inner method 実装。
//!
//! Tavily の検索結果を返す。discover フェーズがまだ追跡していない銘柄・用語・
//! テーマを深掘りするための tool。1 回の戦略タスク実行 (agent 視点の 1 task = 複数 step
//! からなる) あたりの呼び出し回数に上限を設ける。

use core_application::tavily_credit_budget::{
    TavilyCreditBudgetDenial, TavilyCreditBudgetReason, TavilyCreditBudgetScope,
};
use core_application::web_search::{WebSearchTimeRange, WebSearchTopic};
use rmcp::ErrorData as McpError;

use super::dto::{SearchWebArticle, SearchWebParams, SearchWebResult};
use super::{
    StrategyServer, invalid_params, tavily_credit_budget_denial_to_mcp,
    tavily_credit_budget_error_to_mcp,
};

pub(super) const TOOL_NAME: &str = "search_web";

/// 1 回の戦略タスク実行 (`task_execution_id` = `x-execution-id` の `a2a_task_id` 部分) あたりの
/// `search_web` 呼び出し回数上限。
pub(super) const SEARCH_WEB_MAX_CALLS_PER_TASK: u32 = 20;

impl StrategyServer {
    pub(super) async fn search_web_inner(
        &self,
        params: SearchWebParams,
        call_id: String,
        request_fingerprint: String,
        authorization_scope: String,
    ) -> Result<SearchWebResult, McpError> {
        let query = params.query.trim().to_string();
        if query.is_empty() {
            return Err(invalid_params("query must not be empty"));
        }

        let use_cases = self
            .dependencies
            .tavily_credit_budget
            .as_ref()
            .ok_or_else(|| {
                tavily_credit_budget_denial_to_mcp(TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::ConfigurationUnavailable,
                    TavilyCreditBudgetScope::Account,
                    None,
                ))
            })?;

        let outcome = match use_cases
            .search(
                call_id,
                request_fingerprint,
                authorization_scope,
                query,
                params.topic.map(Into::<WebSearchTopic>::into),
                params.time_range.map(Into::<WebSearchTimeRange>::into),
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!("search_web request stopped");
                return Err(tavily_credit_budget_error_to_mcp(error));
            }
        };

        Ok(SearchWebResult {
            results: outcome
                .value
                .into_iter()
                .map(|result| SearchWebArticle {
                    title: result.title,
                    url: result.url,
                    published_date: result.published_date,
                    snippet: result.snippet,
                })
                .collect(),
            retrieved_at: outcome.fetched_at,
        })
    }
}

impl From<super::dto::SearchWebTopic> for WebSearchTopic {
    fn from(topic: super::dto::SearchWebTopic) -> Self {
        match topic {
            super::dto::SearchWebTopic::General => Self::General,
            super::dto::SearchWebTopic::News => Self::News,
        }
    }
}

impl From<super::dto::SearchWebTimeRange> for WebSearchTimeRange {
    fn from(range: super::dto::SearchWebTimeRange) -> Self {
        match range {
            super::dto::SearchWebTimeRange::Day => Self::Day,
            super::dto::SearchWebTimeRange::Week => Self::Week,
            super::dto::SearchWebTimeRange::Month => Self::Month,
            super::dto::SearchWebTimeRange::Year => Self::Year,
        }
    }
}
