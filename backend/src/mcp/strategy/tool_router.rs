//! `#[tool_router]` による tool 登録。
//!
//! 各メソッドは ctx から検証済み `StrategyScope` (と必要なら execution_id) を作り、対応する
//! ドメインモジュールの `*_inner` に委譲するだけの薄いラッパー。tool を追加する際は
//! このファイルにラッパーを追加すること。

use std::borrow::Cow;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use super::dto::{
    CheckBuyableQtyParams, CheckBuyableQtyResult, CreateAnnotationParams, CreateAnnotationResult,
    EvalIndicatorParams, EvalIndicatorResult, EvalPythonParams, EvalPythonResult,
    ListNoteKindsResult, ListNotesParams, ListNotesResult, ListPredictionsParams,
    ListPredictionsResult, NoteDto, QueryDataParams, QueryDataResult, QueryMediaParams,
    QueryMediaResult, ReadAnnotationsParams, ReadAnnotationsResult, ReadCommentsParams,
    ReadCommentsResult, ReadFinSummaryParams, ReadFinSummaryResult, ReadMacroIndicatorParams,
    ReadMacroIndicatorResult, ReadNoteParams, ReadPortfolioResult, ReadPredictionStatsResult,
    ReadSectorShortRatioParams, ReadSectorShortRatioResult, ReadShareholdingStructureParams,
    ReadShareholdingStructureResult, ReadShortSaleReportsParams, ReadShortSaleReportsResult,
    ReadTradesParams, ReadTradesResult, ReadValuationParams, ReadValuationResult,
    RecordPredictionParams, RecordPredictionResult, ReplyCommentParams, ReplyCommentResult,
    ResolveCommentParams, ResolveCommentResult, SearchNewsParams, SearchNewsResult,
    SearchWebParams, SearchWebResult, WriteNoteParams, WriteNoteResult,
};
use super::margin::{ReadMarginParams, ReadMarginResult};
use super::media::TOOL_NAME as QUERY_MEDIA_TOOL_NAME;
use super::ref_terms::{
    AddRefTermsParams, AddRefTermsResult, RemoveRefTermsParams, RemoveRefTermsResult,
};
use super::refs::{SearchRefsParams, SearchRefsResult};
use super::web_search::TOOL_NAME as SEARCH_WEB_TOOL_NAME;
use super::{
    StrategyServer, execution_step_id_from_ctx, execution_task_id_from_ctx, tool_model_from_ctx,
};

#[tool_router]
impl StrategyServer {
    /// 利用できるノート種別を返す
    #[tool(
        name = "list_note_kinds",
        description = "List the available note kinds and whether each kind requires human approval.",
        annotations(read_only_hint = true)
    )]
    async fn list_note_kinds(
        &self,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListNoteKindsResult>, McpError> {
        self.list_note_kinds_inner().await.map(Json)
    }

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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let execution_step_id = execution_step_id_from_ctx(&ctx);
        self.query_data_inner(scope, execution_step_id, params)
            .await
            .map(Json)
    }

    /// ノートを作成または更新する
    #[tool(
        name = "write_note",
        description = "Create a new note or append a version to an existing note owned by the strategy. Supply note_id to update; omit it to create. Set kind only when creating a note. For kinds that require approval, provide change_reason for every version after the first; the new version remains pending until a human approves it. Optionally attach diagrams via graphs (replaces the array wholesale). Idempotent within an execution step, even across a resume: repeated create calls (omitting note_id) for the same step collapse onto a single note instead of creating duplicates."
    )]
    async fn write_note(
        &self,
        Parameters(params): Parameters<WriteNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<WriteNoteResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        // a2a_task_id を含めず execution_step_id 部分のみをキーにする。resume で
        // a2a_task_id (= x-execution-id の task_id 部分) が変わっても、同じステップが
        // 書くノートが 1 件に収束するようにするため。
        let execution_id = execution_step_id_from_ctx(&ctx).map(|id| id.to_string());
        self.write_note_inner(scope, execution_id, params)
            .await
            .map(Json)
    }

    /// ノートを読み出す
    #[tool(
        name = "read_note",
        description = "Read a single note owned by the strategy, including its graphs and linked note versions. Omit version_id to read the current version.",
        annotations(read_only_hint = true)
    )]
    async fn read_note(
        &self,
        Parameters(params): Parameters<ReadNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<NoteDto>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_note_inner(scope, params).await.map(Json)
    }

    /// 戦略のノート一覧を返す (新しい順)
    #[tool(
        name = "list_notes",
        description = "List notes owned by the strategy, newest first. Filter by kind, ref (kind:id), status, and/or updated_after. Set include_pending: true to include notes without a current version, using their latest version. Set include_body: false to omit body_md and save context.",
        annotations(read_only_hint = true)
    )]
    async fn list_notes(
        &self,
        Parameters(params): Parameters<ListNotesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListNotesResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.list_notes_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let execution_step_id = execution_step_id_from_ctx(&ctx);
        let execution_task_id = execution_task_id_from_ctx(&ctx);
        self.create_annotation_inner(scope, execution_step_id, execution_task_id, params)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_annotations_inner(scope, params).await.map(Json)
    }

    /// ノート / アノテーションに付いたレビューコメントを読み出す
    #[tool(
        name = "read_comments",
        description = "List review comments attached to a note version or annotation owned by the strategy, oldest first. Threads are represented via parent_id. Optionally filter by resolved.",
        annotations(read_only_hint = true)
    )]
    async fn read_comments(
        &self,
        Parameters(params): Parameters<ReadCommentsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadCommentsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_comments_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.resolve_comment_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.reply_comment_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.eval_python_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.eval_indicator_inner(scope, params).await.map(Json)
    }

    /// 動画/音声 URL の内容をテキスト化する
    #[tool(
        name = "query_media",
        description = "Fetch a video or audio URL (YouTube links are well supported; other public https:// URLs are best-effort) and answer prompt about its content using the model configured for this tool in agent_graph.tool_models, returning free-form text. Use for source material with no text equivalent, such as a YouTube video.",
        annotations(read_only_hint = true)
    )]
    async fn query_media(
        &self,
        Parameters(params): Parameters<QueryMediaParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<QueryMediaResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let model = tool_model_from_ctx(&ctx, QUERY_MEDIA_TOOL_NAME)?;
        self.query_media_inner(scope, model, params).await.map(Json)
    }

    /// 問い合わせ文で web 検索し、テキストと出典 URL を返す
    #[tool(
        name = "search_web",
        description = "Search the web for a free-form query using the model configured for this tool in agent_graph.tool_models with web search enabled. Returns free-form text plus deduplicated source URLs. Use this to look into stocks, terms, or themes beyond the available reference data / RSS feeds, or to read the actual content of a search_news item beyond its truncated body_snippet (query with the item's title and/or url). Calls are capped per strategy task execution; once the cap is hit, further calls within the same task execution fail with an error.",
        annotations(read_only_hint = true)
    )]
    async fn search_web(
        &self,
        Parameters(params): Parameters<SearchWebParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchWebResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let model = tool_model_from_ctx(&ctx, SEARCH_WEB_TOOL_NAME)?;
        let task_execution_id = execution_task_id_from_ctx(&ctx);
        self.search_web_inner(scope, task_execution_id, model, params)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_portfolio_inner(scope).await.map(Json)
    }

    /// 個々の約定を account-wide (全戦略横断) で返す
    #[tool(
        name = "read_trades",
        description = "Return individual trade executions (date, symbol, side, qty, price) across the entire account, using the same account-wide scope as read_portfolio (not limited to the connecting strategy; each trade carries its own strategy_id). Each trade has a notes array of linked note_id/note_version_id pairs; trades without linked notes have an empty array. For trades belonging to the connecting strategy, pass these as note_id/version_id to read_note to inspect the linked version. Optionally filter by symbol and a lower bound on trade date. Use this to inspect the actual fills behind a past decision, newest first.",
        annotations(read_only_hint = true)
    )]
    async fn read_trades(
        &self,
        Parameters(params): Parameters<ReadTradesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadTradesResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_trades_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.check_buyable_qty_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_shareholding_structure_inner(scope, params)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_short_sale_reports_inner(scope, params)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_sector_short_ratio_inner(scope, params)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_macro_indicator_inner(scope, params)
            .await
            .map(Json)
    }

    /// news_item を title/body_snippet のキーワードと published_at の期間で直接検索する
    #[tool(
        name = "search_news",
        description = "Search news_item directly by keyword (case-insensitive substring match against title or body_snippet) and/or a published_at date range, newest first. body_snippet is truncated to the first 280 characters of the source feed's description, not the full article; use search_web with the title if you need more than that.",
        annotations(read_only_hint = true)
    )]
    async fn search_news(
        &self,
        Parameters(params): Parameters<SearchNewsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchNewsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.search_news_inner(scope, params).await.map(Json)
    }

    /// 参照型 (stock/indicator/sector/theme) を id/name/別名の部分一致で横断検索する
    #[tool(
        name = "search_refs",
        description = "Search across all first-class reference types (stock, indicator, sector, theme) by substring match against id, name, or a registered alias (ref_term), ignoring case and full-width/half-width differences. Returns ref_kind/ref_id/name sorted by name.",
        annotations(read_only_hint = true)
    )]
    async fn search_refs(
        &self,
        Parameters(params): Parameters<SearchRefsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchRefsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.search_refs_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.add_ref_terms_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.remove_ref_terms_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_fin_summary_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_valuation_inner(scope, params).await.map(Json)
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
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_margin_inner(scope, params).await.map(Json)
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
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::services::litellm_client::LiteLlmClient;

    fn mock_db_with_strategy(strategy_id: uuid::Uuid) -> sea_orm::DatabaseConnection {
        use sea_orm::{DatabaseBackend, MockDatabase};

        let row = std::collections::BTreeMap::from([(
            "id".to_string(),
            sea_orm::Value::from(strategy_id),
        )]);
        MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![row]])
            .into_connection()
    }

    fn request_context(
        server: &StrategyServer,
        strategy_id: uuid::Uuid,
        tool_models_header: Option<&str>,
    ) -> (
        RequestContext<RoleServer>,
        rmcp::service::RunningService<RoleServer, StrategyServer>,
    ) {
        use rmcp::model::NumberOrString;
        use rmcp::service::serve_directly;

        let (server_io, _client_io) = tokio::io::duplex(64);
        let (reader, writer) = tokio::io::split(server_io);
        let running = serve_directly::<RoleServer, _, _, std::io::Error, _>(
            server.clone(),
            (reader, writer),
            None,
        );

        let mut parts = axum::http::Request::new(()).into_parts().0;
        parts.headers.insert(
            "x-strategy-id",
            strategy_id.to_string().parse().expect("valid header value"),
        );
        if let Some(tool_models_header) = tool_models_header {
            parts.headers.insert(
                "x-tool-models",
                tool_models_header.parse().expect("valid header value"),
            );
        }
        let mut ctx = RequestContext::new(NumberOrString::Number(1), running.peer().clone());
        ctx.extensions.insert(parts);
        (ctx, running)
    }

    #[tokio::test]
    async fn query_media_uses_model_from_tool_models_header() {
        use crate::mcp::strategy::dto::{QueryMediaParams, QueryMediaResult};

        let strategy_id = uuid::Uuid::new_v4();
        let litellm = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "mock media summary"}}],
            })))
            .mount(&litellm)
            .await;

        let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
        let server = StrategyServer::new(mock_db_with_strategy(strategy_id), None)
            .with_litellm_client(Some(std::sync::Arc::new(client)));
        let (ctx, running) = request_context(
            &server,
            strategy_id,
            Some(r#"{"search_web":"example-model-search","query_media":"example-model-media"}"#),
        );
        let result = server
            .query_media(
                Parameters(QueryMediaParams {
                    media_url: "https://example.com/video.mp4".into(),
                    prompt: "summarize the clip".into(),
                }),
                ctx,
            )
            .await
            .map(|Json(result)| result);
        let requests = litellm
            .received_requests()
            .await
            .expect("recorded requests");
        let request = requests
            .first()
            .expect("handler should send a LiteLLM request");
        let body: serde_json::Value = request.body_json().expect("parse request body");

        assert_eq!(
            (result, body),
            (
                Ok(QueryMediaResult {
                    text: "mock media summary".into(),
                }),
                json!({
                    "model": "example-model-media",
                    "messages": [{
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "summarize the clip"},
                            {"type": "file", "file": {"file_id": "https://example.com/video.mp4"}},
                        ],
                    }],
                }),
            ),
        );

        running.cancel().await.expect("stop the test server");
    }

    #[tokio::test]
    async fn search_web_uses_model_from_tool_models_header() {
        use indoc::indoc;

        use crate::mcp::strategy::dto::{SearchWebParams, SearchWebResult};

        let strategy_id = uuid::Uuid::new_v4();
        let litellm = MockServer::start().await;
        let response_body = format!(
            indoc! {"
                data: {}

                data: [DONE]

            "},
            json!({"choices": [{"delta": {"content": "mock search result"}}]})
        );
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
            .mount(&litellm)
            .await;

        let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
        let server = StrategyServer::new(mock_db_with_strategy(strategy_id), None)
            .with_litellm_client(Some(std::sync::Arc::new(client)));
        let (ctx, running) = request_context(
            &server,
            strategy_id,
            Some(r#"{"search_web":"example-model-search","query_media":"example-model-media"}"#),
        );
        let result = server
            .search_web(
                Parameters(SearchWebParams {
                    query: "example query".into(),
                }),
                ctx,
            )
            .await
            .map(|Json(result)| result);
        let requests = litellm
            .received_requests()
            .await
            .expect("recorded requests");
        let request = requests
            .first()
            .expect("handler should send a LiteLLM request");
        let body: serde_json::Value = request.body_json().expect("parse request body");

        assert_eq!(
            (result, body),
            (
                Ok(SearchWebResult {
                    text: "mock search result".into(),
                    citations: vec![],
                }),
                json!({
                    "model": "example-model-search",
                    "messages": [{
                        "role": "user",
                        "content": [{"type": "text", "text": "example query"}],
                    }],
                    "stream": true,
                    "web_search_options": {},
                    "allowed_openai_params": ["web_search_options"],
                }),
            ),
        );

        running.cancel().await.expect("stop the test server");
    }

    #[tokio::test]
    async fn query_data_rejects_nonexistent_strategy() {
        use sea_orm::{DatabaseBackend, MockDatabase};
        use uuid::Uuid;

        let strategy_id = Uuid::new_v4();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<gateway_postgres::entities::strategy::Model>::new()])
            .into_connection();
        let server = StrategyServer::new(db, None);
        let (ctx, running) = request_context(&server, strategy_id, None);

        assert_eq!(
            server
                .query_data(
                    Parameters(QueryDataParams {
                        instrument_ids: Vec::new(),
                        from: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid test date"),
                        to: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid test date"),
                    }),
                    ctx,
                )
                .await
                .map(|Json(result)| result),
            Err(McpError::invalid_params(
                format!("strategy {strategy_id} not found"),
                None,
            )),
        );

        running.cancel().await.expect("stop the test server");
    }

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
                ("add_ref_terms", None),
                ("check_buyable_qty", Some(true)),
                ("create_annotation", None),
                ("eval_indicator", None),
                ("eval_python", None),
                ("list_note_kinds", Some(true)),
                ("list_notes", Some(true)),
                ("list_predictions", Some(true)),
                ("query_data", Some(true)),
                ("query_media", Some(true)),
                ("read_annotations", Some(true)),
                ("read_comments", Some(true)),
                ("read_fin_summary", Some(true)),
                ("read_macro_indicator", Some(true)),
                ("read_margin", Some(true)),
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
