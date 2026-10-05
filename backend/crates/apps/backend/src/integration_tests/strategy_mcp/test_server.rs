use std::ops::Deref;

use axum::http::{HeaderMap, HeaderValue};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use super::super::mcp_tool::call_tool_output_with_headers;
use super::dto as strategy_dto;
use entrypoint_agent_mcp::StrategyServer as EntrypointStrategyServer;

use super::test_api::{earnings_targets, ref_terms, refs, stock_groups, stock_registration};

#[derive(Clone)]
pub(super) struct StrategyServer {
    server: EntrypointStrategyServer,
}

impl StrategyServer {
    pub(super) fn new(server: EntrypointStrategyServer) -> Self {
        Self { server }
    }

    pub(super) fn with_kata_executor(
        mut self,
        kata_executor: Option<core_application::kata_exec::SharedKataExecutor>,
    ) -> Self {
        self.server = self.server.with_kata_executor(kata_executor);
        self
    }

    pub(super) fn with_litellm_client(
        mut self,
        llm_client: Option<core_application::llm_client::SharedLlmClient>,
    ) -> Self {
        self.server = self.server.with_litellm_client(llm_client);
        self
    }

    pub(super) async fn invoke<TInput, TOutput>(
        &self,
        name: &'static str,
        strategy_id: Uuid,
        input: TInput,
        execution_id: Option<String>,
        model: Option<String>,
    ) -> Result<ToolOutput<TOutput>, McpError>
    where
        TInput: Serialize,
        TOutput: DeserializeOwned + Serialize,
    {
        let arguments = serde_json::to_value(input)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-strategy-id",
            HeaderValue::from_str(&strategy_id.to_string()).expect("UUID header is valid"),
        );
        if let Some(execution_id) = execution_id {
            headers.insert(
                "x-execution-id",
                HeaderValue::from_str(&execution_id).expect("execution ID header is valid"),
            );
        }
        if let Some(model) = model {
            let models = match name {
                "query_youtube" => serde_json::json!({"query_youtube": model}),
                "search_web" => serde_json::json!({"search_web": model}),
                _ => serde_json::json!({}),
            };
            headers.insert(
                "x-tool-models",
                HeaderValue::from_str(&models.to_string()).expect("tool models header is valid"),
            );
        }
        let raw = call_tool_output_with_headers::<_, Value>(&self.server, name, arguments, headers)
            .await?;
        let value = serde_json::from_value(raw.clone())
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(ToolOutput { raw, value })
    }

    fn execution_header(step_id: Option<Uuid>, task_id: Option<String>) -> Option<String> {
        step_id.map(|step_id| {
            format!(
                "{}:{step_id}",
                task_id.unwrap_or_else(|| "test-task".into())
            )
        })
    }

    pub(super) async fn query_data(
        &self,
        scope: impl Into<StrategyScope>,
        execution_step_id: Option<Uuid>,
        params: strategy_dto::QueryDataParams,
    ) -> Result<ToolOutput<strategy_dto::QueryDataResult>, McpError> {
        self.invoke(
            "query_data",
            scope.into().id(),
            params,
            Self::execution_header(execution_step_id, None),
            None,
        )
        .await
    }

    pub(super) async fn write_note(
        &self,
        scope: impl Into<StrategyScope>,
        execution_step_id: Option<Uuid>,
        params: strategy_dto::WriteNoteParams,
    ) -> Result<ToolOutput<strategy_dto::WriteNoteResult>, McpError> {
        let execution_id = execution_step_id.map(|step_id| format!("test-task:{step_id}"));
        self.invoke("write_note", scope.into().id(), params, execution_id, None)
            .await
    }

    pub(super) async fn read_note(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadNoteParams,
    ) -> Result<ToolOutput<strategy_dto::NoteDto>, McpError> {
        self.invoke("read_note", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn list_notes(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ListNotesParams,
    ) -> Result<ToolOutput<strategy_dto::ListNotesResult>, McpError> {
        self.invoke("list_notes", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn create_annotation(
        &self,
        scope: impl Into<StrategyScope>,
        execution_step_id: Option<Uuid>,
        execution_task_id: Option<String>,
        params: strategy_dto::CreateAnnotationParams,
    ) -> Result<ToolOutput<strategy_dto::CreateAnnotationResult>, McpError> {
        self.invoke(
            "create_annotation",
            scope.into().id(),
            params,
            Self::execution_header(execution_step_id, execution_task_id),
            None,
        )
        .await
    }

    pub(super) async fn read_annotations(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadAnnotationsParams,
    ) -> Result<ToolOutput<strategy_dto::ReadAnnotationsResult>, McpError> {
        self.invoke("read_annotations", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn read_comments(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadCommentsParams,
    ) -> Result<ToolOutput<strategy_dto::ReadCommentsResult>, McpError> {
        self.invoke("read_comments", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn resolve_comment(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ResolveCommentParams,
    ) -> Result<ToolOutput<strategy_dto::ResolveCommentResult>, McpError> {
        self.invoke("resolve_comment", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn reply_comment(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReplyCommentParams,
    ) -> Result<ToolOutput<strategy_dto::ReplyCommentResult>, McpError> {
        self.invoke("reply_comment", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn eval_python(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::EvalPythonParams,
    ) -> Result<ToolOutput<strategy_dto::EvalPythonResult>, McpError> {
        self.invoke("eval_python", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn eval_indicator(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::EvalIndicatorParams,
    ) -> Result<ToolOutput<strategy_dto::EvalIndicatorResult>, McpError> {
        self.invoke("eval_indicator", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn query_youtube(
        &self,
        scope: impl Into<StrategyScope>,
        model: String,
        params: strategy_dto::QueryYoutubeParams,
    ) -> Result<ToolOutput<strategy_dto::QueryYoutubeResult>, McpError> {
        self.invoke(
            "query_youtube",
            scope.into().id(),
            params,
            None,
            Some(model),
        )
        .await
    }

    pub(super) async fn search_web(
        &self,
        scope: impl Into<StrategyScope>,
        task_execution_id: Option<String>,
        model: String,
        params: strategy_dto::SearchWebParams,
    ) -> Result<ToolOutput<strategy_dto::SearchWebResult>, McpError> {
        let execution_id = task_execution_id.map(|task_id| format!("{task_id}:{}", Uuid::new_v4()));
        self.invoke(
            "search_web",
            scope.into().id(),
            params,
            execution_id,
            Some(model),
        )
        .await
    }

    pub(super) async fn read_portfolio(
        &self,
        scope: impl Into<StrategyScope>,
    ) -> Result<ToolOutput<strategy_dto::ReadPortfolioResult>, McpError> {
        self.invoke(
            "read_portfolio",
            scope.into().id(),
            serde_json::json!({}),
            None,
            None,
        )
        .await
    }

    pub(super) async fn read_trades(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadTradesParams,
    ) -> Result<ToolOutput<strategy_dto::ReadTradesResult>, McpError> {
        self.invoke("read_trades", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn check_buyable_qty(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::CheckBuyableQtyParams,
    ) -> Result<ToolOutput<strategy_dto::CheckBuyableQtyResult>, McpError> {
        self.invoke("check_buyable_qty", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn read_shareholding_structure(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadShareholdingStructureParams,
    ) -> Result<ToolOutput<strategy_dto::ReadShareholdingStructureResult>, McpError> {
        self.invoke(
            "read_shareholding_structure",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn read_short_sale_reports(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadShortSaleReportsParams,
    ) -> Result<ToolOutput<strategy_dto::ReadShortSaleReportsResult>, McpError> {
        self.invoke(
            "read_short_sale_reports",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn read_sector_short_ratio(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadSectorShortRatioParams,
    ) -> Result<ToolOutput<strategy_dto::ReadSectorShortRatioResult>, McpError> {
        self.invoke(
            "read_sector_short_ratio",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn read_macro_indicator(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ReadMacroIndicatorParams,
    ) -> Result<ToolOutput<strategy_dto::ReadMacroIndicatorResult>, McpError> {
        self.invoke(
            "read_macro_indicator",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn search_news(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::SearchNewsParams,
    ) -> Result<ToolOutput<strategy_dto::SearchNewsResult>, McpError> {
        self.invoke("search_news", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn get_news_content(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::GetNewsContentParams,
    ) -> Result<ToolOutput<strategy_dto::GetNewsContentResult>, McpError> {
        self.invoke("get_news_content", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn search_refs(
        &self,
        scope: impl Into<StrategyScope>,
        params: refs::SearchRefsParams,
    ) -> Result<ToolOutput<refs::SearchRefsResult>, McpError> {
        self.invoke("search_refs", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn add_ref_terms(
        &self,
        scope: impl Into<StrategyScope>,
        params: ref_terms::AddRefTermsParams,
    ) -> Result<ToolOutput<ref_terms::AddRefTermsResult>, McpError> {
        self.invoke("add_ref_terms", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn remove_ref_terms(
        &self,
        scope: impl Into<StrategyScope>,
        params: ref_terms::RemoveRefTermsParams,
    ) -> Result<ToolOutput<ref_terms::RemoveRefTermsResult>, McpError> {
        self.invoke("remove_ref_terms", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn record_prediction(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::RecordPredictionParams,
    ) -> Result<ToolOutput<strategy_dto::RecordPredictionResult>, McpError> {
        self.invoke("record_prediction", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn list_predictions(
        &self,
        scope: impl Into<StrategyScope>,
        params: strategy_dto::ListPredictionsParams,
    ) -> Result<ToolOutput<strategy_dto::ListPredictionsResult>, McpError> {
        self.invoke("list_predictions", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn read_prediction_stats(
        &self,
        scope: impl Into<StrategyScope>,
    ) -> Result<ToolOutput<strategy_dto::ReadPredictionStatsResult>, McpError> {
        self.invoke(
            "read_prediction_stats",
            scope.into().id(),
            serde_json::json!({}),
            None,
            None,
        )
        .await
    }

    pub(super) async fn create_stock_group(
        &self,
        scope: impl Into<StrategyScope>,
        params: stock_groups::CreateStockGroupParams,
    ) -> Result<ToolOutput<stock_groups::StockGroupDto>, McpError> {
        self.invoke("create_stock_group", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn register_stock(
        &self,
        scope: impl Into<StrategyScope>,
        params: stock_registration::RegisterStockParams,
    ) -> Result<ToolOutput<stock_registration::RegisterStockResult>, McpError> {
        self.invoke("register_stock", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn update_stock_group(
        &self,
        scope: impl Into<StrategyScope>,
        params: stock_groups::UpdateStockGroupParams,
    ) -> Result<ToolOutput<stock_groups::StockGroupDto>, McpError> {
        self.invoke("update_stock_group", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn add_stock_to_group(
        &self,
        scope: impl Into<StrategyScope>,
        params: stock_groups::StockGroupMemberParams,
    ) -> Result<ToolOutput<stock_groups::StockGroupMemberChangeResult>, McpError> {
        self.invoke("add_stock_to_group", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn remove_stock_from_group(
        &self,
        scope: impl Into<StrategyScope>,
        params: stock_groups::StockGroupMemberParams,
    ) -> Result<ToolOutput<stock_groups::StockGroupMemberChangeResult>, McpError> {
        self.invoke(
            "remove_stock_from_group",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn list_stock_group_members(
        &self,
        scope: impl Into<StrategyScope>,
        params: stock_groups::ListStockGroupMembersParams,
    ) -> Result<ToolOutput<stock_groups::ListStockGroupMembersResult>, McpError> {
        self.invoke(
            "list_stock_group_members",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn add_earnings_target(
        &self,
        scope: impl Into<StrategyScope>,
        params: earnings_targets::EarningsTargetParams,
    ) -> Result<ToolOutput<earnings_targets::EarningsTargetChangeResult>, McpError> {
        self.invoke("add_earnings_target", scope.into().id(), params, None, None)
            .await
    }

    pub(super) async fn remove_earnings_target(
        &self,
        scope: impl Into<StrategyScope>,
        params: earnings_targets::EarningsTargetParams,
    ) -> Result<ToolOutput<earnings_targets::EarningsTargetChangeResult>, McpError> {
        self.invoke(
            "remove_earnings_target",
            scope.into().id(),
            params,
            None,
            None,
        )
        .await
    }

    pub(super) async fn list_earnings_targets(
        &self,
        scope: impl Into<StrategyScope>,
    ) -> Result<ToolOutput<earnings_targets::ListEarningsTargetsResult>, McpError> {
        self.invoke(
            "list_earnings_targets",
            scope.into().id(),
            serde_json::json!({}),
            None,
            None,
        )
        .await
    }
}

/// MCP の JSON を保持し、DTO では無視される追加 field も比較対象にする。
#[derive(Clone, Debug)]
pub(super) struct ToolOutput<T> {
    raw: Value,
    value: T,
}

impl<T: DeserializeOwned> ToolOutput<T> {
    pub(super) fn normalize_json(mut self, normalize: impl FnOnce(&mut Value)) -> Self {
        normalize(&mut self.raw);
        self.value = serde_json::from_value(self.raw.clone())
            .expect("normalization preserves the tool output shape");
        self
    }

    pub(super) fn into_value(self) -> T {
        self.value
    }

    pub(super) fn as_json(&self) -> &Value {
        &self.raw
    }
}

impl<T> Deref for ToolOutput<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Serialize> PartialEq<T> for ToolOutput<T> {
    fn eq(&self, other: &T) -> bool {
        serde_json::to_value(other).is_ok_and(|value| self.raw == value)
    }
}
