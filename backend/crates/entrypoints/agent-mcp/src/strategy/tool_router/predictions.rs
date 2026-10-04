use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{
    ListPredictionsParams, ListPredictionsResult, ReadPredictionStatsResult,
    RecordPredictionParams, RecordPredictionResult,
};

#[tool_router(router = predictions_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 予測を記録する (書き込み専用。更新・削除 tool は存在しない)
    #[tool(
        name = "record_prediction",
        description = "Record a prediction that target_stock_id will outperform or underperform benchmark_stock_id (measured from base_date's close to due_date) with a fixed-step probability (0.55/0.6/0.65/0.7/0.75/0.8/0.85/0.9). Write-once: there is no update or delete tool, since changing a recorded prediction would invalidate later grading."
    )]
    async fn record_prediction(
        &self,
        Parameters(params): Parameters<RecordPredictionParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<RecordPredictionResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.record_prediction_inner(scope, params).await.map(Json)
    }

    /// 接続元戦略が記録した予測を一覧する
    #[tool(
        name = "list_predictions",
        description = "List predictions recorded by the current strategy, newest first. Filter by due_after/due_before (e.g. due_after=today to see only predictions not yet graded).",
        annotations(read_only_hint = true)
    )]
    async fn list_predictions(
        &self,
        Parameters(params): Parameters<ListPredictionsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListPredictionsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.list_predictions_inner(scope, params).await.map(Json)
    }

    /// 接続元戦略の採点済み予測を Brier score と確率刻みごとの的中率で集計する
    #[tool(
        name = "read_prediction_stats",
        description = "Return calibration stats for the current strategy's graded predictions: the Brier score (mean squared error between each prediction's recorded probability and its 0/1 outcome; lower is better-calibrated) and per-probability-step count/hit_rate (hit_rate is null for steps with zero graded predictions). Predictions are graded automatically once their due_date's daily bar has been ingested; ungraded predictions are excluded entirely, so graded_count can be smaller than the total number of predictions recorded so far.",
        annotations(read_only_hint = true)
    )]
    async fn read_prediction_stats(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadPredictionStatsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_prediction_stats_inner(scope).await.map(Json)
    }
}
