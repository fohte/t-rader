//! `search_web` tool の inner method 実装。
//!
//! Tavily の検索結果を返す。discover フェーズがまだ追跡していない銘柄・用語・
//! テーマを深掘りするための tool。1 回の戦略タスク実行 (agent 視点の 1 task = 複数 step
//! からなる) あたりの呼び出し回数に上限を設ける。

use core_application::mcp_tool_call_count::McpToolCallCountUseCaseError;
use core_application::strategy_scope::StrategyScope;
use core_application::web_search::{WebSearchError, WebSearchTimeRange, WebSearchTopic};
use rmcp::ErrorData as McpError;

use super::dto::{SearchWebArticle, SearchWebParams, SearchWebResult};
use super::{StrategyServer, internal_error, invalid_params};

pub(super) const TOOL_NAME: &str = "search_web";

/// 1 回の戦略タスク実行 (`task_execution_id` = `x-execution-id` の `a2a_task_id` 部分) あたりの
/// `search_web` 呼び出し回数上限。
pub(super) const SEARCH_WEB_MAX_CALLS_PER_TASK: u32 = 20;

impl StrategyServer {
    pub(crate) async fn search_web_inner(
        &self,
        scope: impl Into<StrategyScope>,
        task_execution_id: Option<String>,
        params: SearchWebParams,
    ) -> Result<SearchWebResult, McpError> {
        let session_strategy_id = scope.into().id();
        let query = params.query.trim().to_string();
        if query.is_empty() {
            return Err(invalid_params("query must not be empty"));
        }

        let client = self
            .dependencies
            .web_search_client
            .as_ref()
            .ok_or_else(|| internal_error("TAVILY_API_KEY is not configured"))?;
        let tool_call_counts = &self.dependencies.mcp_tool_call_counts;

        // task_execution_id はヘッダ欠落時 (手動呼び出し等) に None になる。その場合は
        // 呼び出し回数の追跡をスキップし、fail-open で検索を実行する。
        if let Some(task_execution_id) = task_execution_id.as_deref() {
            tool_call_counts
                .reserve(task_execution_id, TOOL_NAME, SEARCH_WEB_MAX_CALLS_PER_TASK)
                .await
                .map_err(tool_call_count_error_to_mcp)?;
        }

        tracing::info!(
            strategy_id = %session_strategy_id,
            query,
            "search_web: dispatching web search request"
        );

        let outcome = match client
            .search(
                &query,
                params.topic.map(Into::<WebSearchTopic>::into),
                params.time_range.map(Into::<WebSearchTimeRange>::into),
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(
                    strategy_id = %session_strategy_id,
                    query,
                    error = %error,
                    "search_web: web search request failed"
                );
                // 検索が実際には行われなかったので、予約した呼び出し回数を戻す。
                if let Some(task_execution_id) = task_execution_id.as_deref()
                    && let Err(err) = tool_call_counts.release(task_execution_id, TOOL_NAME).await
                {
                    tracing::warn!(
                        task_execution_id,
                        tool_name = TOOL_NAME,
                        error = %err,
                        "search_web: failed to release call count reservation after a failed request"
                    );
                }
                return Err(web_search_error_to_mcp(error));
            }
        };

        Ok(SearchWebResult {
            results: outcome
                .into_iter()
                .map(|result| SearchWebArticle {
                    title: result.title,
                    url: result.url,
                    published_date: result.published_date,
                    snippet: result.snippet,
                })
                .collect(),
        })
    }
}

fn web_search_error_to_mcp(error: WebSearchError) -> McpError {
    internal_error(format!("web search failed: {error}"))
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

fn tool_call_count_error_to_mcp(error: McpToolCallCountUseCaseError) -> McpError {
    match error {
        error @ McpToolCallCountUseCaseError::CallLimitExceeded { .. } => {
            invalid_params(error.to_string())
        }
        error => {
            tracing::error!(error = %error, "strategy mcp db error");
            internal_error(format!("database error: {error}"))
        }
    }
}
