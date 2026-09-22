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
    ListNotesParams, ListNotesResult, ListPredictionsParams, ListPredictionsResult,
    ListWatchTargetsParams, ListWatchTargetsResult, NoteDto, ProposeHypothesisChangeParams,
    ProposeHypothesisChangeResult, QueryDataParams, QueryDataResult, QueryMediaParams,
    QueryMediaResult, ReadAnnotationsParams, ReadAnnotationsResult, ReadCommentsParams,
    ReadCommentsResult, ReadFinSummaryParams, ReadFinSummaryResult, ReadHypothesisParams,
    ReadMacroIndicatorParams, ReadMacroIndicatorResult, ReadNewsParams, ReadNewsResult,
    ReadNoteParams, ReadPortfolioResult, ReadPredictionStatsResult, ReadSectorShortRatioParams,
    ReadSectorShortRatioResult, ReadShareholdingStructureParams, ReadShareholdingStructureResult,
    ReadShortSaleReportsParams, ReadShortSaleReportsResult, ReadTradesParams, ReadTradesResult,
    ReadValuationParams, ReadValuationResult, RecordPredictionParams, RecordPredictionResult,
    ReplyCommentParams, ReplyCommentResult, ResolveCommentParams, ResolveCommentResult,
    SearchNewsParams, SearchNewsResult, SearchWebParams, SearchWebResult, WriteNoteParams,
    WriteNoteResult,
};
use super::margin::{ReadMarginParams, ReadMarginResult};
use super::ref_terms::{
    AddRefTermsParams, AddRefTermsResult, RemoveRefTermsParams, RemoveRefTermsResult,
};
use super::refs::{SearchRefsParams, SearchRefsResult};
use super::{
    StrategyServer, execution_id_from_ctx, execution_step_id_from_ctx, execution_task_id_from_ctx,
    strategy_id_from_ctx,
};

#[tool_router]
impl StrategyServer {
    /// 複数銘柄 + 期間で日足バーデータをまとめて取得する
    #[tool(
        name = "query_data",
        description = "Fetch daily OHLCV bars for one or more instruments (up to 100 per call, no duplicates) over a shared date range from the DB. Results are in the same order as instrument_ids; an instrument with no ingested data returns an empty bars array rather than an error.",
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
        description = "Create a new note or update an existing note owned by the strategy. Supply note_id to update; omit it to create. Optionally attach diagrams via graphs (replaces the array wholesale). Idempotent within an execution step, even across a resume: repeated create calls (omitting note_id) for the same step collapse onto a single note instead of creating duplicates."
    )]
    async fn write_note(
        &self,
        Parameters(params): Parameters<WriteNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<WriteNoteResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        // a2a_task_id を含めず execution_step_id 部分のみをキーにする。resume で
        // a2a_task_id (= x-execution-id の task_id 部分) が変わっても、同じステップが
        // 書くノートが 1 件に収束するようにするため。
        let execution_id = execution_step_id_from_ctx(&ctx).map(|id| id.to_string());
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
        description = "Create a chart annotation owned by the strategy. On a resume, an unread annotation created by an earlier attempt of the same execution step is replaced; already-reviewed ones are kept."
    )]
    async fn create_annotation(
        &self,
        Parameters(params): Parameters<CreateAnnotationParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<CreateAnnotationResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        let execution_step_id = execution_step_id_from_ctx(&ctx);
        let execution_task_id = execution_task_id_from_ctx(&ctx);
        self.create_annotation_inner(sid, execution_step_id, execution_task_id, params)
            .await
            .map(Json)
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
        description = "Add a derived interest (role=derived, origin=llm) to the current strategy. Idempotent: returns created=false if the same (ref_kind, ref_id) already exists for the strategy. If ref_id doesn't match a master id but uniquely matches a registered alias, it is resolved to the canonical id before being stored."
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
        description = "Search the web for a free-form query using an LLM with web search enabled (configured via the WEB_SEARCH_MODEL env var). Returns free-form text plus deduplicated source URLs. Use this to look into stocks, terms, or themes not yet tracked by add_interest / RSS feeds, or to read the actual content of a read_news / search_news item beyond its truncated body_snippet (query with the item's title and/or url). Calls are capped per strategy task execution; once the cap is hit, further calls within the same task execution fail with an error.",
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

    /// 個々の約定を account-wide (全戦略横断) で返す
    #[tool(
        name = "read_trades",
        description = "Return individual trade executions (date, symbol, side, qty, price) across the entire account, using the same account-wide scope as read_portfolio (not limited to the connecting strategy; each trade carries its own strategy_id). Optionally filter by symbol and a lower bound on trade date. Use this to inspect the actual fills behind a past decision, newest first.",
        annotations(read_only_hint = true)
    )]
    async fn read_trades(
        &self,
        Parameters(params): Parameters<ReadTradesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadTradesResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_trades_inner(sid, params).await.map(Json)
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

    /// 銘柄の空売り残高報告を新しい順に返す
    #[tool(
        name = "read_short_sale_reports",
        description = "Read a stock's short-sale position reports (J-Quants /markets/short-sale-report), newest disclosure date first (ties broken by reporter name). Only positions of 0.5% or more of shares outstanding are reportable, so an empty result means no reportable short position, not necessarily no short position at all. Each row is one reporter's report for one disclosure date; short_position_ratio and prev_report_ratio are fractions (e.g. 0.01 = 1%) so their difference is the change since that reporter's previous report (prev_report_ratio/prev_report_date are null on a reporter's first report). symbol is the 4-digit code (matched against the 5-digit J-Quants code by its leading 4 characters). from/to filter by disclosure date (inclusive) and are both optional.",
        annotations(read_only_hint = true)
    )]
    async fn read_short_sale_reports(
        &self,
        Parameters(params): Parameters<ReadShortSaleReportsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadShortSaleReportsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_short_sale_reports_inner(sid, params)
            .await
            .map(Json)
    }

    /// 業種別の空売りの売買代金と空売り比率を日ごとに返す
    #[tool(
        name = "read_sector_short_ratio",
        description = "Read a sector's daily short-selling turnover value and short ratio (J-Quants /markets/short-ratio), newest date first. sector is the same 33-sector name used by the sector table / search_refs / check_buyable_qty (e.g. \"輸送用機器\"); unrecognized names are rejected. Each day reports sell_excluding_short_value (non-short sell orders), short_with_restriction_value and short_without_restriction_value (short sell orders, split by whether the uptick price restriction applied), all in yen, plus the derived short_ratio (short turnover / total sell turnover, a fraction, e.g. 0.1 = 10%). All four fields are null on a day with no trading in that sector. from/to filter by date (inclusive) and are both optional.",
        annotations(read_only_hint = true)
    )]
    async fn read_sector_short_ratio(
        &self,
        Parameters(params): Parameters<ReadSectorShortRatioParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadSectorShortRatioResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_sector_short_ratio_inner(sid, params)
            .await
            .map(Json)
    }

    /// マクロ指標 (ドル円, VIX, 米10年債利回り, 日経225 等) の日次観測値を期間指定で返す
    #[tool(
        name = "read_macro_indicator",
        description = "Read daily observations (date + value) for a macro indicator between from and to (inclusive), oldest first. Discover available indicator_id values via search_refs (ref_kind=indicator), e.g. USDJPY, VIX, US10Y, NIKKEI225. Values are in the source's native units (USDJPY: yen per dollar, VIX: index level, US10Y: percent). Days with no observation (holidays, no update) are simply absent rather than interpolated; USDJPY in particular is batched weekly at the source and can lag by up to about a week, so the last item's date shows how fresh the latest available value is. Returns an empty list if the indicator_id is unknown or has no data in range.",
        annotations(read_only_hint = true)
    )]
    async fn read_macro_indicator(
        &self,
        Parameters(params): Parameters<ReadMacroIndicatorParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadMacroIndicatorResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_macro_indicator_inner(sid, params).await.map(Json)
    }

    /// 戦略に紐づく未読ニュースを checkpoint 以降分だけ返す
    #[tool(
        name = "read_news",
        description = "Read news items linked to the strategy that haven't been returned by a previous call, oldest first. A per-strategy checkpoint automatically advances past whatever this call returns, so repeated calls only surface items linked since the last call — nothing is skipped even across long gaps between runs. Each row is one interest match; a news item matched by more than one interest (e.g. a stock and a theme) appears once per match, so the same url/title can repeat. If has_more is true, call again to continue from where this call left off. body_snippet is truncated to the first 280 characters of the source feed's description, not the full article; use search_web with the title if you need more than that."
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
        description = "Search news_item directly by keyword (case-insensitive substring match against title or body_snippet) and/or a published_at date range, newest first. Unlike read_news, this ignores news_strategy_link entirely, so results are not affected by whether the strategy has registered a matching interest term. body_snippet is truncated to the first 280 characters of the source feed's description, not the full article; use search_web with the title if you need more than that.",
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

    /// 参照型 (stock/indicator/sector/theme) を id/name/別名の部分一致で横断検索する
    #[tool(
        name = "search_refs",
        description = "Search across all first-class reference types (stock, indicator, sector, theme) by substring match against id, name, or a registered alias (ref_term), ignoring case and full-width/half-width differences. Returns ref_kind/ref_id/name sorted by name, usable directly as input to add_interest.",
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

    /// 参照型に別名 (表記揺れ・略称・旧社名等) を追加する
    #[tool(
        name = "add_ref_terms",
        description = "Add aliases (alternate spellings, abbreviations, former names, etc.) to a first-class reference (stock/indicator/sector/theme). Idempotent: terms already registered for the same (ref_kind, ref_id) are silently skipped and excluded from the returned added list. Blank terms are ignored."
    )]
    async fn add_ref_terms(
        &self,
        Parameters(params): Parameters<AddRefTermsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<AddRefTermsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.add_ref_terms_inner(sid, params).await.map(Json)
    }

    /// 参照型から別名を削除する
    #[tool(
        name = "remove_ref_terms",
        description = "Remove aliases from a first-class reference (stock/indicator/sector/theme). Idempotent: terms not currently registered are silently skipped and excluded from the returned removed list."
    )]
    async fn remove_ref_terms(
        &self,
        Parameters(params): Parameters<RemoveRefTermsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<RemoveRefTermsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.remove_ref_terms_inner(sid, params).await.map(Json)
    }

    /// 銘柄の財務情報 (決算短信の実績・会社予想、業績予想/配当予想の修正) を新しい順に返す
    #[tool(
        name = "read_fin_summary",
        description = "Read a stock's financial disclosures (J-Quants /fins/summary): actual results and company forecasts from earnings reports, plus earnings/dividend forecast revisions, newest first. Quarterly progress rates are decimal ratios of cumulative actuals to the current fiscal year's full-year company forecasts in the same disclosure row (0.5 = 50%); they are null for annual statements and forecast revisions, and when the forecast is missing or <= 0. symbol is the 4-digit code (matched against the 5-digit J-Quants code by its leading 4 characters). When the same disclosure period and document type appears more than once (e.g. a correction), only the one with the highest disclosure number is returned. Fields not reported by the filer (e.g. ordinary_profit under IFRS/US GAAP) are null.",
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

    /// 銘柄の日次バリュエーション指標を新しい順に返す
    #[tool(
        name = "read_valuation",
        description = "Read daily J-Quants valuation indicators for a stock, newest first. symbol is the 4-digit code and matches the leading 4 characters of the 5-digit J-Quants code; from/to are inclusive. roe and fwd_roe are decimal ratios, not percentages. mkt_cap is in millions of yen. Indicators J-Quants cannot calculate are null.",
        annotations(read_only_hint = true)
    )]
    async fn read_valuation(
        &self,
        Parameters(params): Parameters<ReadValuationParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadValuationResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_valuation_inner(sid, params).await.map(Json)
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
        let sid = strategy_id_from_ctx(&ctx)?;
        self.record_prediction_inner(sid, params).await.map(Json)
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
        let sid = strategy_id_from_ctx(&ctx)?;
        self.list_predictions_inner(sid, params).await.map(Json)
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
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_prediction_stats_inner(sid).await.map(Json)
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
                ("add_ref_terms", None),
                ("check_buyable_qty", Some(true)),
                ("create_annotation", None),
                ("eval_indicator", None),
                ("eval_python", None),
                ("list_hypotheses", Some(true)),
                ("list_notes", Some(true)),
                ("list_predictions", Some(true)),
                ("list_watch_targets", Some(true)),
                ("propose_hypothesis_change", None),
                ("query_data", Some(true)),
                ("query_media", Some(true)),
                ("read_annotations", Some(true)),
                ("read_comments", Some(true)),
                ("read_fin_summary", Some(true)),
                ("read_hypothesis", Some(true)),
                ("read_macro_indicator", Some(true)),
                ("read_margin", Some(true)),
                ("read_news", None),
                ("read_note", Some(true)),
                ("read_portfolio", Some(true)),
                ("read_prediction_stats", Some(true)),
                ("read_sector_short_ratio", Some(true)),
                ("read_shareholding_structure", Some(true)),
                ("read_short_sale_reports", Some(true)),
                ("read_trades", Some(true)),
                ("read_valuation", Some(true)),
                ("record_prediction", None),
                ("remove_ref_terms", None),
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
