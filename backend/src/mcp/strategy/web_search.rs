//! `search_web` tool の inner method 実装。
//!
//! 問い合わせ文を web 検索対応モデルに渡し、テキストと出典 URL を返す。discover フェーズが
//! まだ追跡していない銘柄・用語・テーマを深掘りするための tool。1 回の戦略タスク実行
//! (agent 視点の 1 task = 複数 step からなる) あたりの呼び出し回数に上限を設け、
//! 超えたら LiteLLM を呼ばずにエラーを返す。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{DatabaseConnection, EntityTrait, ExprTrait};
use uuid::Uuid;

use crate::entities::mcp_tool_call_count;

use super::dto::{SearchWebParams, SearchWebResult};
use super::{StrategyServer, db_error, internal_error, invalid_params, litellm_error_to_mcp};

const TOOL_NAME: &str = "search_web";

/// 1 回の戦略タスク実行 (`task_execution_id` = `x-execution-id` の `a2a_task_id` 部分) あたりの
/// `search_web` 呼び出し回数上限。
const SEARCH_WEB_MAX_CALLS_PER_TASK: u32 = 20;

/// デフォルトは ChatGPT Plus 経由で web search が有効なモデル。`WEB_SEARCH_MODEL` で
/// モデル名を上書きできる (Gemini 等の他プロバイダに切り替えるため)。
fn web_search_model() -> String {
    std::env::var("WEB_SEARCH_MODEL").unwrap_or_else(|_| "chatgpt/gpt-5.6-luna".to_string())
}

/// `(task_execution_id, tool_name)` の呼び出し回数をアトミックにインクリメントし、
/// インクリメント後の件数を返す。
async fn increment_task_tool_call_count(
    db: &DatabaseConnection,
    task_execution_id: &str,
    tool_name: &str,
) -> Result<i32, McpError> {
    let model = mcp_tool_call_count::ActiveModel {
        id: NotSet,
        task_execution_id: Set(task_execution_id.to_string()),
        tool_name: Set(tool_name.to_string()),
        call_count: Set(1),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let row = mcp_tool_call_count::Entity::insert(model)
        .on_conflict(
            OnConflict::columns([
                mcp_tool_call_count::Column::TaskExecutionId,
                mcp_tool_call_count::Column::ToolName,
            ])
            .value(
                mcp_tool_call_count::Column::CallCount,
                Expr::col((
                    mcp_tool_call_count::Entity,
                    mcp_tool_call_count::Column::CallCount,
                ))
                .add(1),
            )
            .update_column(mcp_tool_call_count::Column::UpdatedAt)
            .to_owned(),
        )
        .exec_with_returning(db)
        .await
        .map_err(db_error)?;
    Ok(row.call_count)
}

impl StrategyServer {
    pub(crate) async fn search_web_inner(
        &self,
        session_strategy_id: Uuid,
        task_execution_id: Option<String>,
        params: SearchWebParams,
    ) -> Result<SearchWebResult, McpError> {
        let query = params.query.trim().to_string();
        if query.is_empty() {
            return Err(invalid_params("query must not be empty"));
        }

        let client = self
            .litellm_client
            .as_ref()
            .ok_or_else(|| internal_error("litellm client is not configured"))?;

        // task_execution_id はヘッダ欠落時 (手動呼び出し等) に None になる。その場合は
        // 呼び出し回数の追跡をスキップし、fail-open で検索を実行する。
        if let Some(task_execution_id) = task_execution_id.as_deref() {
            let call_count =
                increment_task_tool_call_count(&self.db, task_execution_id, TOOL_NAME).await?;
            if call_count > SEARCH_WEB_MAX_CALLS_PER_TASK as i32 {
                return Err(invalid_params(format!(
                    "search_web call limit ({SEARCH_WEB_MAX_CALLS_PER_TASK}) exceeded for this task execution"
                )));
            }
        }

        let model = web_search_model();
        tracing::info!(
            strategy_id = %session_strategy_id,
            model,
            query,
            "search_web: dispatching web search request"
        );

        let outcome = client.web_search(&model, &query).await.map_err(|e| {
            tracing::warn!(
                strategy_id = %session_strategy_id,
                model,
                query,
                error = %e,
                "search_web: web search request failed"
            );
            litellm_error_to_mcp(e)
        })?;

        Ok(SearchWebResult {
            text: outcome.text,
            citations: outcome.citations,
        })
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;
    use sqlx::PgPool;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::services::litellm_client::LiteLlmClient;
    use crate::testing::create_test_db;

    use super::super::StrategyServer;
    use super::super::dto::{SearchWebParams, SearchWebResult};
    use super::*;

    fn mock_db() -> sea_orm::DatabaseConnection {
        MockDatabase::new(DatabaseBackend::Postgres).into_connection()
    }

    fn params(query: &str) -> SearchWebParams {
        SearchWebParams {
            query: query.into(),
        }
    }

    fn sse_body(content: &str) -> String {
        format!(
            indoc! {"
                data: {}

                data: [DONE]

            "},
            json!({"choices": [{"delta": {"content": content}}]})
        )
    }

    #[tokio::test]
    async fn search_web_inner_requires_litellm_client() {
        let server = StrategyServer::new(mock_db(), None);
        let err = server
            .search_web_inner(Uuid::new_v4(), None, params("半導体 関連ニュース"))
            .await
            .expect_err("expected internal error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "litellm client is not configured",
            ),
        );
    }

    #[tokio::test]
    async fn search_web_inner_rejects_empty_query() {
        let server = StrategyServer::new(mock_db(), None);
        let err = server
            .search_web_inner(Uuid::new_v4(), None, params("   "))
            .await
            .expect_err("expected invalid params");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "query must not be empty",
            ),
        );
    }

    #[tokio::test]
    async fn search_web_inner_returns_text_and_citations() {
        let litellm = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body("半導体銘柄が上昇")))
            .mount(&litellm)
            .await;

        let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
        let server = StrategyServer::new(mock_db(), None).with_litellm_client(Some(client));

        let out = server
            .search_web_inner(Uuid::new_v4(), None, params("半導体 関連ニュース"))
            .await
            .expect("search_web");
        assert_eq!(
            out,
            SearchWebResult {
                text: "半導体銘柄が上昇".into(),
                citations: vec![],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn search_web_inner_enforces_per_task_call_limit(pool: PgPool) {
        let db = create_test_db(pool).await;

        let litellm = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body("ok")))
            .mount(&litellm)
            .await;

        let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
        let server = StrategyServer::new(db, None).with_litellm_client(Some(client));
        let task_execution_id = format!("task-{}", Uuid::new_v4());

        for _ in 0..SEARCH_WEB_MAX_CALLS_PER_TASK {
            server
                .search_web_inner(
                    Uuid::new_v4(),
                    Some(task_execution_id.clone()),
                    params("query"),
                )
                .await
                .expect("call within limit should succeed");
        }

        let err = server
            .search_web_inner(
                Uuid::new_v4(),
                Some(task_execution_id.clone()),
                params("query"),
            )
            .await
            .expect_err("call beyond limit should fail");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!(
                    "search_web call limit ({SEARCH_WEB_MAX_CALLS_PER_TASK}) exceeded for this task execution"
                )
                .as_str(),
            ),
        );

        let requests = litellm
            .received_requests()
            .await
            .expect("recorded requests");
        assert_eq!(requests.len(), SEARCH_WEB_MAX_CALLS_PER_TASK as usize);
    }

    #[sqlx::test(migrations = false)]
    async fn increment_task_tool_call_count_is_independent_per_task_and_tool(pool: PgPool) {
        let db = create_test_db(pool).await;
        let task_a = format!("task-{}", Uuid::new_v4());
        let task_b = format!("task-{}", Uuid::new_v4());

        let a_search_1 = increment_task_tool_call_count(&db, &task_a, "search_web")
            .await
            .expect("increment");
        let a_search_2 = increment_task_tool_call_count(&db, &task_a, "search_web")
            .await
            .expect("increment");
        let a_other_tool_1 = increment_task_tool_call_count(&db, &task_a, "other_tool")
            .await
            .expect("increment");
        let b_search_1 = increment_task_tool_call_count(&db, &task_b, "search_web")
            .await
            .expect("increment");

        assert_eq!(
            (a_search_1, a_search_2, a_other_tool_1, b_search_1),
            (1, 2, 1, 1),
        );
    }
}
