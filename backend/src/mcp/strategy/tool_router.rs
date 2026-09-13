//! `#[tool_router]` による tool 登録。
//!
//! 各メソッドは ctx から strategy_id (と必要なら execution_id) を取り出し、対応する
//! ドメインモジュールの `*_inner` に委譲するだけの薄いラッパー。tool を追加する際は
//! このファイルにラッパーを追加すること。

use std::borrow::Cow;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use super::dto::{
    AddInterestParams, AddInterestResult, CheckBuyableQtyParams, CheckBuyableQtyResult,
    CreateAnnotationParams, CreateAnnotationResult, EvalIndicatorParams, EvalIndicatorResult,
    EvalPythonParams, EvalPythonResult, HypothesisDto, ListHypothesesParams, ListHypothesesResult,
    ListNotesParams, ListNotesResult, ListWatchTargetsParams, ListWatchTargetsResult, NoteDto,
    ProposeHypothesisChangeParams, ProposeHypothesisChangeResult, QueryDataParams, QueryDataResult,
    QueryMediaParams, QueryMediaResult, ReadAnnotationsParams, ReadAnnotationsResult,
    ReadCommentsParams, ReadCommentsResult, ReadFinSummaryParams, ReadFinSummaryResult,
    ReadHypothesisParams, ReadMarginParams, ReadMarginResult, ReadNewsParams, ReadNewsResult,
    ReadNoteParams, ReadPortfolioResult, ReadShareholdingStructureParams,
    ReadShareholdingStructureResult, ReplyCommentParams, ReplyCommentResult, ResolveCommentParams,
    ResolveCommentResult, SearchNewsParams, SearchNewsResult, SearchWebParams, SearchWebResult,
    WriteNoteParams, WriteNoteResult,
};
use super::refs::{SearchRefsParams, SearchRefsResult};
use super::{
    StrategyServer, execution_id_from_ctx, execution_step_id_from_ctx, execution_task_id_from_ctx,
    strategy_id_from_ctx,
};

#[tool_router]
impl StrategyServer {
    /// 銘柄 + 期間で日足バーデータを取得する
    #[tool(
        name = "query_data",
        description = "Fetch daily OHLCV bars for an instrument over a date range via the configured data provider.",
        annotations(read_only_hint = true)
    )]
    async fn query_data(
        &self,
        Parameters(params): Parameters<QueryDataParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<QueryDataResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        let execution_step_id = execution_step_id_from_ctx(&ctx);
        self.query_data_inner(sid, execution_step_id, params)
            .await
            .map(Json)
    }

    /// ノートを作成または更新する
    #[tool(
        name = "write_note",
        description = "Create a new note or update an existing note owned by the strategy. Supply note_id to update; omit it to create. Optionally attach diagrams via graphs (replaces the array wholesale). Idempotent within a task execution: repeated create calls (omitting note_id) collapse onto a single note instead of creating duplicates."
    )]
    async fn write_note(
        &self,
        Parameters(params): Parameters<WriteNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<WriteNoteResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        let execution_id = execution_id_from_ctx(&ctx);
        self.write_note_inner(sid, execution_id, params)
            .await
            .map(Json)
    }

    /// ノートを読み出す
    #[tool(
        name = "read_note",
        description = "Read a single note owned by the strategy, including its graphs.",
        annotations(read_only_hint = true)
    )]
    async fn read_note(
        &self,
        Parameters(params): Parameters<ReadNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<NoteDto>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_note_inner(sid, params).await.map(Json)
    }

    /// 戦略のノート一覧を返す (新しい順)
    #[tool(
        name = "list_notes",
        description = "List notes owned by the strategy, newest first. Filter by status and/or updated_after, and set include_body: false to omit body_md and save context.",
        annotations(read_only_hint = true)
    )]
    async fn list_notes(
        &self,
        Parameters(params): Parameters<ListNotesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListNotesResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.list_notes_inner(sid, params).await.map(Json)
    }

    /// アノテーションを作成する
    #[tool(
        name = "create_annotation",
        description = "Create a chart annotation owned by the strategy."
    )]
    async fn create_annotation(
        &self,
        Parameters(params): Parameters<CreateAnnotationParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<CreateAnnotationResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.create_annotation_inner(sid, params).await.map(Json)
    }

    /// 戦略のアノテーション一覧を返す
    #[tool(
        name = "read_annotations",
        description = "List annotations owned by the strategy. Optionally filter by target_symbol.",
        annotations(read_only_hint = true)
    )]
    async fn read_annotations(
        &self,
        Parameters(params): Parameters<ReadAnnotationsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadAnnotationsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_annotations_inner(sid, params).await.map(Json)
    }

    /// ノート / アノテーションに付いたレビューコメントを読み出す
    #[tool(
        name = "read_comments",
        description = "List review comments attached to a note or annotation owned by the strategy, oldest first. Threads are represented via parent_id. Optionally filter by resolved.",
        annotations(read_only_hint = true)
    )]
    async fn read_comments(
        &self,
        Parameters(params): Parameters<ReadCommentsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadCommentsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_comments_inner(sid, params).await.map(Json)
    }

    /// レビューコメントを解決済み/未解決に切り替える
    #[tool(
        name = "resolve_comment",
        description = "Mark a review comment owned by the strategy as resolved or unresolved."
    )]
    async fn resolve_comment(
        &self,
        Parameters(params): Parameters<ResolveCommentParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ResolveCommentResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.resolve_comment_inner(sid, params).await.map(Json)
    }

    /// レビューコメントに返信する
    #[tool(
        name = "reply_comment",
        description = "Reply to an existing review comment owned by the strategy. Posted with author_kind=llm, author_label=analyst."
    )]
    async fn reply_comment(
        &self,
        Parameters(params): Parameters<ReplyCommentParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReplyCommentResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.reply_comment_inner(sid, params).await.map(Json)
    }

    /// Python コードを exec Pod で実行する
    #[tool(
        name = "eval_python",
        description = "Run a Python snippet inside an isolated Kata Containers exec Pod and return stdout/stderr/exit_code. Network, subprocess, and persistent filesystem are denied."
    )]
    async fn eval_python(
        &self,
        Parameters(params): Parameters<EvalPythonParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<EvalPythonResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.eval_python_inner(sid, params).await.map(Json)
    }

    /// 戦略 Agent が新しい関心 (derived / origin=llm 固定) を追加する
    #[tool(
        name = "add_interest",
        description = "Add a derived interest (role=derived, origin=llm) to the current strategy. Idempotent: returns created=false if the same (ref_kind, ref_id) already exists for the strategy."
    )]
    async fn add_interest(
        &self,
        Parameters(params): Parameters<AddInterestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<AddInterestResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.add_interest_inner(sid, params).await.map(Json)
    }

    /// 人間が「追う」と決めた監視対象銘柄一覧を返す (保有状況によるフィルタは行わない)
    #[tool(
        name = "list_watch_targets",
        description = "List stocks a human has marked to watch for the current strategy (origin=human, status=active), oldest first. Excludes interests the agent added itself (origin=llm) and archived ones. Not pre-filtered against current holdings; combine with read_portfolio / check_buyable_qty as needed.",
        annotations(read_only_hint = true)
    )]
    async fn list_watch_targets(
        &self,
        Parameters(params): Parameters<ListWatchTargetsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListWatchTargetsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.list_watch_targets_inner(sid, params).await.map(Json)
    }

    /// 永続化された indicator (戦略 scope 優先) を exec Pod 上で評価する
    #[tool(
        name = "eval_indicator",
        description = "Evaluate a stored indicator by name. Resolves strategy-scoped indicator first then global. Args are validated against the indicator's input_schema and stdout is validated against output_schema."
    )]
    async fn eval_indicator(
        &self,
        Parameters(params): Parameters<EvalIndicatorParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<EvalIndicatorResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.eval_indicator_inner(sid, params).await.map(Json)
    }

    /// 動画/音声 URL の内容を Gemini でテキスト化する
    #[tool(
        name = "query_media",
        description = "Fetch a video or audio URL (YouTube links are well supported; other public https:// URLs are best-effort) and answer prompt about its content via Gemini, returning free-form text. Use for source material with no text equivalent, such as a YouTube video.",
        annotations(read_only_hint = true)
    )]
    async fn query_media(
        &self,
        Parameters(params): Parameters<QueryMediaParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<QueryMediaResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.query_media_inner(sid, params).await.map(Json)
    }

    /// 問い合わせ文で web 検索し、テキストと出典 URL を返す
    #[tool(
        name = "search_web",
        description = "Search the web for a free-form query using an LLM with web search enabled (defaults to a ChatGPT Plus-backed model; override the model via the WEB_SEARCH_MODEL env var, e.g. to switch to Gemini). Returns free-form text plus deduplicated source URLs. Use this to look into stocks, terms, or themes not yet tracked by add_interest / RSS feeds. Calls are capped per strategy task execution; once the cap is hit, further calls within the same task execution fail with an error.",
        annotations(read_only_hint = true)
    )]
    async fn search_web(
        &self,
        Parameters(params): Parameters<SearchWebParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchWebResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        let task_execution_id = execution_task_id_from_ctx(&ctx);
        self.search_web_inner(sid, task_execution_id, params)
            .await
            .map(Json)
    }

    /// 口座全体 (全戦略横断) の保有銘柄と実現損益、および接続元戦略のスライスを時価で返す
    #[tool(
        name = "read_portfolio",
        description = "Return account-wide open positions and realized P&L (FIFO) aggregated across all strategies, plus the connecting strategy's own slice, both priced at current market value. Use this to check existing holdings and available investable amount before proposing new trades.",
        annotations(read_only_hint = true)
    )]
    async fn read_portfolio(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadPortfolioResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_portfolio_inner(sid).await.map(Json)
    }

    /// 指定銘柄をあと何株買えるかを、制約ごとの上限株数とともに返す
    #[tool(
        name = "check_buyable_qty",
        description = "Calculate how many more shares of a symbol can be bought, per constraint (account-wide sector ratio cap and the strategy's remaining unused investable amount), plus the overall minimum and which constraint is binding. Works for symbols not currently held (current_qty is 0). max_additional_qty values are floored to 100-share lots (see lot_size). A constraint with no configured cap reports status=unlimited; a constraint that cannot be computed (missing price, missing sector, no investable amount recorded) reports status=unavailable with a reason instead of a possibly-wrong number, and poisons the overall max_qty to unavailable too.",
        annotations(read_only_hint = true)
    )]
    async fn check_buyable_qty(
        &self,
        Parameters(params): Parameters<CheckBuyableQtyParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<CheckBuyableQtyResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.check_buyable_qty_inner(sid, params).await.map(Json)
    }

    /// 銘柄の保有構造 (大量保有報告書・大株主状況・政策保有株式) を返す
    #[tool(
        name = "read_shareholding_structure",
        description = "Return the ownership structure of a stock (given as a 4-digit symbol) from ingested EDINET filings: large-volume shareholding reports and amendments for the stock (large_volume_reports, newest first, each with document_type, change_reason for amendments, total_shares_ratio/total_shares_ratio_last, and per-holder holding_purpose/shares_ratio/shares_ratio_last — all ratios are fractions, e.g. 0.1 = 10%); the most recent major-shareholders filing's top holders (major_shareholders, ranked); and the company's own most recent cross-shareholding (policy holdings) disclosure, per counterparty (cross_shareholdings, with current/previous shares and book value plus mutual_holding indicating whether the counterparty reciprocally holds this company's stock). Matches EDINET's 5-digit code by its first 4 characters against symbol, since the exact correspondence isn't documented by J-Quants. major_shareholders/cross_shareholdings are null and large_volume_reports is empty when no matching filing has been ingested.",
        annotations(read_only_hint = true)
    )]
    async fn read_shareholding_structure(
        &self,
        Parameters(params): Parameters<ReadShareholdingStructureParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadShareholdingStructureResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_shareholding_structure_inner(sid, params)
            .await
            .map(Json)
    }

    /// 戦略に紐づく未読ニュースを checkpoint 以降分だけ返す
    #[tool(
        name = "read_news",
        description = "Read news items linked to the strategy that haven't been returned by a previous call, oldest first. A per-strategy checkpoint automatically advances past whatever this call returns, so repeated calls only surface items linked since the last call — nothing is skipped even across long gaps between runs. Each row is one interest match; a news item matched by more than one interest (e.g. a stock and a theme) appears once per match, so the same url/title can repeat. If has_more is true, call again to continue from where this call left off."
    )]
    async fn read_news(
        &self,
        Parameters(params): Parameters<ReadNewsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadNewsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        let execution_id = execution_id_from_ctx(&ctx);
        self.read_news_inner(sid, execution_id, params)
            .await
            .map(Json)
    }

    /// news_item を title/body_snippet のキーワードと published_at の期間で直接検索する
    #[tool(
        name = "search_news",
        description = "Search news_item directly by keyword (case-insensitive substring match against title or body_snippet) and/or a published_at date range, newest first. Unlike read_news, this ignores news_strategy_link entirely, so results are not affected by whether the strategy has registered a matching interest term.",
        annotations(read_only_hint = true)
    )]
    async fn search_news(
        &self,
        Parameters(params): Parameters<SearchNewsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchNewsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.search_news_inner(sid, params).await.map(Json)
    }

    /// 参照型 (stock/indicator/sector/theme) を id/name の部分一致で横断検索する
    #[tool(
        name = "search_refs",
        description = "Search across all first-class reference types (stock, indicator, sector, theme) by case-insensitive substring match against id or name. Returns ref_kind/ref_id/name sorted by name, usable directly as input to add_interest.",
        annotations(read_only_hint = true)
    )]
    async fn search_refs(
        &self,
        Parameters(params): Parameters<SearchRefsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchRefsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.search_refs_inner(sid, params).await.map(Json)
    }

    /// 銘柄の財務情報 (決算短信の実績・会社予想、業績予想/配当予想の修正) を新しい順に返す
    #[tool(
        name = "read_fin_summary",
        description = "Read a stock's financial disclosures (J-Quants /fins/summary): actual results and company forecasts from earnings reports, plus earnings/dividend forecast revisions, newest first. symbol is the 4-digit code (matched against the 5-digit J-Quants code by its leading 4 characters). When the same disclosure period and document type appears more than once (e.g. a correction), only the one with the highest disclosure number is returned. Fields not reported by the filer (e.g. ordinary_profit under IFRS/US GAAP) are null.",
        annotations(read_only_hint = true)
    )]
    async fn read_fin_summary(
        &self,
        Parameters(params): Parameters<ReadFinSummaryParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadFinSummaryResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_fin_summary_inner(sid, params).await.map(Json)
    }

    /// 銘柄の信用残 (信用取引週末残高/信用取引残高、日々公表信用取引残高) を返す
    #[tool(
        name = "read_margin",
        description = "Read a stock's margin trading balances: weekly (later daily) margin interest balances (margin_interest) newest first, tagged with the 5-digit J-Quants code and iss_type (1=margin-eligible, 2=loan-eligible, 3=other), plus daily-published margin balances (margin_alert, only for stocks the exchange has designated for daily publication — absence from this list does not mean a zero balance) with pub_reason flags and tse_mrgn_reg_cls. When the same application date has multiple corrections, only the one with the latest publication date is returned. symbol is the 4-digit code (matched against the 5-digit J-Quants code by its leading 4 characters); from/to filter by date (inclusive) and default to no bound.",
        annotations(read_only_hint = true)
    )]
    async fn read_margin(
        &self,
        Parameters(params): Parameters<ReadMarginParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadMarginResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_margin_inner(sid, params).await.map(Json)
    }

    /// 接続元戦略の仮説 + account-wide (global) 仮説を一覧する
    #[tool(
        name = "list_hypotheses",
        description = "List hypotheses visible to the current strategy: hypotheses owned by this strategy plus account-wide (global) hypotheses, newest first.",
        annotations(read_only_hint = true)
    )]
    async fn list_hypotheses(
        &self,
        Parameters(params): Parameters<ListHypothesesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListHypothesesResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.list_hypotheses_inner(sid, params).await.map(Json)
    }

    /// 単一の仮説を読む (自戦略または global)
    #[tool(
        name = "read_hypothesis",
        description = "Read a single hypothesis (its title, body, and status) visible to the current strategy (own or global).",
        annotations(read_only_hint = true)
    )]
    async fn read_hypothesis(
        &self,
        Parameters(params): Parameters<ReadHypothesisParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<HypothesisDto>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_hypothesis_inner(sid, params).await.map(Json)
    }

    /// 仮説へのタイトル/本文/status の変更を提案として永続化する (仮説本体には反映しない)
    #[tool(
        name = "propose_hypothesis_change",
        description = "Propose a change to a hypothesis's title, body, and/or status, with a rationale. The proposal is persisted but not applied — a human must approve it via the API before the hypothesis itself is updated. The agent cannot write to hypotheses directly."
    )]
    async fn propose_hypothesis_change(
        &self,
        Parameters(params): Parameters<ProposeHypothesisChangeParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ProposeHypothesisChangeResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.propose_hypothesis_change_inner(sid, params)
            .await
            .map(Json)
    }
}

impl StrategyServer {
    /// tool 一覧を (name, description) で返す。`#[tool(...)]` の登録情報をそのまま使うので、
    /// tool を追加してもここを手で更新する必要はない。
    pub(crate) fn list_tool_summaries() -> Vec<(String, Option<String>)> {
        Self::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| {
                (
                    tool.name.into_owned(),
                    tool.description.map(Cow::into_owned),
                )
            })
            .collect()
    }
}

#[tool_handler]
impl ServerHandler for StrategyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("t-rader-strategy", env!("CARGO_PKG_VERSION")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_hint_matches_read_write_split() {
        let read_only_hints: std::collections::BTreeMap<String, Option<bool>> =
            StrategyServer::tool_router()
                .list_all()
                .into_iter()
                .map(|tool| {
                    (
                        tool.name.into_owned(),
                        tool.annotations.and_then(|a| a.read_only_hint),
                    )
                })
                .collect();

        assert_eq!(
            read_only_hints,
            [
                ("add_interest", None),
                ("check_buyable_qty", Some(true)),
                ("create_annotation", None),
                ("eval_indicator", None),
                ("eval_python", None),
                ("list_hypotheses", Some(true)),
                ("list_notes", Some(true)),
                ("list_watch_targets", Some(true)),
                ("propose_hypothesis_change", None),
                ("query_data", Some(true)),
                ("query_media", Some(true)),
                ("read_annotations", Some(true)),
                ("read_comments", Some(true)),
                ("read_fin_summary", Some(true)),
                ("read_hypothesis", Some(true)),
                ("read_margin", Some(true)),
                ("read_news", None),
                ("read_note", Some(true)),
                ("read_portfolio", Some(true)),
                ("read_shareholding_structure", Some(true)),
                ("reply_comment", None),
                ("resolve_comment", None),
                ("search_news", Some(true)),
                ("search_refs", Some(true)),
                ("search_web", Some(true)),
                ("write_note", None),
            ]
            .into_iter()
            .map(|(name, hint)| (name.to_string(), hint))
            .collect::<std::collections::BTreeMap<_, _>>(),
        );
    }

    /// 生成された JSON Schema が MCP クライアント (zod ベースの SDK) に拒否される裸の
    /// boolean スキーマを含まないことの回帰テスト。`serde_json::Value` 型のフィールドが
    /// 将来追加されても機械的に検出できる。
    #[test]
    fn tool_schemas_have_no_boolean_property_schemas() {
        for tool in StrategyServer::tool_router().list_all() {
            crate::mcp::assert_no_boolean_property_schemas(&tool);
        }
    }
}
