use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStrategyRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateStrategyRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}

/// フローティングチャットから戦略 Agent に投入する 1 メッセージ。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyChatRequest {
    #[schema(min_length = 1)]
    pub prompt: String,
}

/// `POST /api/strategies/:id/chat` の戻り値。後続の polling 用 task 識別子を返す。
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyChatResponse {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

/// `GET /api/strategies/:id/tasks/:task_id` の戻り値。
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyTaskStatusResponse {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub source: String,
    pub prompt: String,
    pub phase: String,
    pub error_summary: Option<String>,
    /// agent の最終応答テキスト (completed 時のみ)
    pub result_text: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    /// フェーズ/分岐ごとの実行状況。`strategy_task_step` の各行から既知のフィールドのみを
    /// 再構築した配列。
    #[schema(value_type = serde_json::Value)]
    pub steps: serde_json::Value,
    /// 投入時に指定された purpose。タスクは常に purpose キーの実行グラフでフェーズを
    /// 表示する。`None` はこのカラムが追加される前に作成された行に限られる。
    pub purpose: Option<String>,
}

/// `GET /api/strategies/:id/tasks` と `GET /api/tasks` の一覧要素。`steps`/`result_text`
/// は一覧では使わないため含めない。
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyTaskSummary {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub source: String,
    pub prompt: String,
    pub phase: String,
    pub error_summary: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    /// 投入時に指定された purpose。`None` は purpose カラム追加前に作成された行に限られる。
    pub purpose: Option<String>,
}

/// 戦略の投資可能額を新しい history 行として記録するリクエスト。
/// 口座の現金残高や証券会社の買付余力とは別概念で、ユーザーが投資に回すと決めた枠を表す。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutInvestableAmountRequest {
    pub amount_jpy: Decimal,
    /// 省略時はサーバー側で現在時刻を使う
    #[serde(default)]
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub effective_at: Option<DateTime<FixedOffset>>,
}

/// 戦略の現在有効な投資可能額 (`effective_at` が現在時刻以下の最新行)。
/// history が 1 行も無い戦略では両方 null。
#[derive(Debug, Serialize, ToSchema)]
pub struct InvestableAmountResponse {
    pub amount_jpy: Option<Decimal>,
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub effective_at: Option<DateTime<FixedOffset>>,
}
