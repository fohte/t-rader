use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::mcp::StrategyServer;
use crate::models::{AgentModelsResponse, AgentTool, AgentToolsResponse};

/// 戦略 Agent 設定フォームに供給するモデル一覧を取得する。
/// LLM ゲートウェイが未設定、または応答不能な場合は空配列を返す (設定画面全体を壊さないため)。
#[utoipa::path(
    get,
    path = "/api/agent-models",
    tag = "agent_options",
    responses((status = 200, body = AgentModelsResponse)),
)]
pub async fn get_agent_models(State(state): State<AppState>) -> Json<AgentModelsResponse> {
    let models = match &state.llm_gateway_client {
        Some(client) => client.list_models().await.unwrap_or_else(|e| {
            tracing::warn!(
                error = %e,
                "failed to fetch agent models from llm gateway; returning empty list"
            );
            Vec::new()
        }),
        None => Vec::new(),
    };
    Json(AgentModelsResponse { models })
}

/// 戦略 MCP の tool 一覧を取得する。`#[tool(...)]` の登録情報から動的に組み立てるので、
/// tool を追加してもここを手で更新する必要はない。
#[utoipa::path(
    get,
    path = "/api/agent-tools",
    tag = "agent_options",
    responses((status = 200, body = AgentToolsResponse)),
)]
pub async fn get_agent_tools() -> Json<AgentToolsResponse> {
    let tools = StrategyServer::list_tool_summaries()
        .into_iter()
        .map(|(name, description)| AgentTool { name, description })
        .collect();
    Json(AgentToolsResponse { tools })
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::{create_test_server, create_test_server_with_llm_gateway};

    #[sqlx::test(migrations = false)]
    async fn agent_models_returns_empty_list_when_llm_gateway_unconfigured(pool: PgPool) {
        let server = create_test_server(pool).await;
        let response = server.get("/api/agent-models").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({ "models": [] }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_models_proxies_llm_gateway_response(pool: PgPool) {
        let llm_gateway = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/model_group/info"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": [
                        {
                            "model_group": "claude-opus-4",
                            "providers": ["anthropic"],
                            "supports_reasoning": true,
                        },
                    ],
                })),
            )
            .mount(&llm_gateway)
            .await;

        let server = create_test_server_with_llm_gateway(pool, &llm_gateway.uri()).await;
        let response = server.get("/api/agent-models").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({
                "models": [
                    {
                        "id": "claude-opus-4",
                        "providers": ["anthropic"],
                        "max_input_tokens": null,
                        "max_output_tokens": null,
                        "supports_reasoning": true,
                    },
                ],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_models_returns_empty_list_when_llm_gateway_unreachable(pool: PgPool) {
        let llm_gateway = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/model_group/info"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&llm_gateway)
            .await;

        let server = create_test_server_with_llm_gateway(pool, &llm_gateway.uri()).await;
        let response = server.get("/api/agent-models").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({ "models": [] }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_tools_lists_known_strategy_mcp_tools(pool: PgPool) {
        let server = create_test_server(pool).await;
        let response = server.get("/api/agent-tools").await;
        response.assert_status_ok();
        // ToolRouter::list_all() は name の昇順でソートして返す
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({
                "tools": [
                    {"name": "add_interest", "description": "Add a derived interest (role=derived, origin=llm) to the current strategy. Idempotent: returns created=false if the same (ref_kind, ref_id) already exists for the strategy."},
                    {"name": "check_buyable_qty", "description": "Calculate how many more shares of a symbol can be bought, per constraint (account-wide sector ratio cap and the strategy's remaining unused investable amount), plus the overall minimum and which constraint is binding. Works for symbols not currently held (current_qty is 0). max_additional_qty values are floored to 100-share lots (see lot_size). A constraint with no configured cap reports status=unlimited; a constraint that cannot be computed (missing price, missing sector, no investable amount recorded) reports status=unavailable with a reason instead of a possibly-wrong number, and poisons the overall max_qty to unavailable too."},
                    {"name": "create_annotation", "description": "Create a chart annotation owned by the strategy."},
                    {"name": "eval_indicator", "description": "Evaluate a stored indicator by name. Resolves strategy-scoped indicator first then global. Args are validated against the indicator's input_schema and stdout is validated against output_schema."},
                    {"name": "eval_python", "description": "Run a Python snippet inside an isolated Kata Containers exec Pod and return stdout/stderr/exit_code. Network, subprocess, and persistent filesystem are denied."},
                    {"name": "list_hypotheses", "description": "List hypotheses visible to the current strategy: hypotheses owned by this strategy plus account-wide (global) hypotheses, newest first."},
                    {"name": "list_notes", "description": "List notes owned by the strategy, newest first."},
                    {"name": "list_watch_targets", "description": "List stocks a human has marked to watch for the current strategy (origin=human, status=active), oldest first. Excludes interests the agent added itself (origin=llm) and archived ones. Not pre-filtered against current holdings; combine with read_portfolio / check_buyable_qty as needed."},
                    {"name": "propose_hypothesis_change", "description": "Propose a change to a hypothesis's title, body, and/or status, with a rationale. The proposal is persisted but not applied — a human must approve it via the API before the hypothesis itself is updated. The agent cannot write to hypotheses directly."},
                    {"name": "query_data", "description": "Fetch daily OHLCV bars for an instrument over a date range via the configured data provider."},
                    {"name": "query_media", "description": "Fetch a video or audio URL (YouTube links are well supported; other public https:// URLs are best-effort) and answer prompt about its content via Gemini, returning free-form text. Use for source material with no text equivalent, such as a YouTube video."},
                    {"name": "read_annotations", "description": "List annotations owned by the strategy. Optionally filter by target_symbol."},
                    {"name": "read_comments", "description": "List review comments attached to a note or annotation owned by the strategy, oldest first. Threads are represented via parent_id. Optionally filter by resolved."},
                    {"name": "read_hypothesis", "description": "Read a single hypothesis (its title, body, and status) visible to the current strategy (own or global)."},
                    {"name": "read_news", "description": "Read news items linked to the strategy that haven't been returned by a previous call, oldest first. A per-strategy checkpoint automatically advances past whatever this call returns, so repeated calls only surface items linked since the last call — nothing is skipped even across long gaps between runs. Each row is one interest match; a news item matched by more than one interest (e.g. a stock and a theme) appears once per match, so the same url/title can repeat. If has_more is true, call again to continue from where this call left off."},
                    {"name": "read_note", "description": "Read a single note owned by the strategy, including its graphs."},
                    {"name": "read_portfolio", "description": "Return account-wide open positions and realized P&L (FIFO) aggregated across all strategies, plus the connecting strategy's own slice, both priced at current market value. Use this to check existing holdings and available investable amount before proposing new trades."},
                    {"name": "reply_comment", "description": "Reply to an existing review comment owned by the strategy. Posted with author_kind=llm, author_label=analyst."},
                    {"name": "resolve_comment", "description": "Mark a review comment owned by the strategy as resolved or unresolved."},
                    {"name": "search_news", "description": "Search news_item directly by keyword (case-insensitive substring match against title or body_snippet) and/or a published_at date range, newest first. Unlike read_news, this ignores news_strategy_link entirely, so results are not affected by whether the strategy has registered a matching interest term."},
                    {"name": "search_refs", "description": "Search across all first-class reference types (stock, indicator, sector, theme) by case-insensitive substring match against id or name. Returns ref_kind/ref_id/name sorted by name, usable directly as input to add_interest."},
                    {"name": "search_web", "description": "Search the web for a free-form query using an LLM with web search enabled (defaults to a ChatGPT Plus-backed model; override the model via the WEB_SEARCH_MODEL env var, e.g. to switch to Gemini). Returns free-form text plus deduplicated source URLs. Use this to look into stocks, terms, or themes not yet tracked by add_interest / RSS feeds. Calls are capped per strategy task execution; once the cap is hit, further calls within the same task execution fail with an error."},
                    {"name": "write_note", "description": "Create a new note or update an existing note owned by the strategy. Supply note_id to update; omit it to create. Optionally attach diagrams via graphs (replaces the array wholesale). Idempotent within a task execution: repeated create calls (omitting note_id) collapse onto a single note instead of creating duplicates."},
                ],
            }),
        );
    }
}
