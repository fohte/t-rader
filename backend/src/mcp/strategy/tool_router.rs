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
    AddInterestParams, AddInterestResult, CreateAnnotationParams, CreateAnnotationResult,
    EvalIndicatorParams, EvalIndicatorResult, EvalPythonParams, EvalPythonResult, ListNotesParams,
    ListNotesResult, NoteDto, QueryDataParams, QueryDataResult, QueryMediaParams, QueryMediaResult,
    ReadAnnotationsParams, ReadAnnotationsResult, ReadCommentsParams, ReadCommentsResult,
    ReadNoteParams, ReplyCommentParams, ReplyCommentResult, ResolveCommentParams,
    ResolveCommentResult, WriteNoteParams, WriteNoteResult,
};
use super::{StrategyServer, execution_id_from_ctx, strategy_id_from_ctx};

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
        self.query_data_inner(sid, params).await.map(Json)
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
        description = "List notes owned by the strategy, newest first.",
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
                ("create_annotation", None),
                ("eval_indicator", None),
                ("eval_python", None),
                ("list_notes", Some(true)),
                ("query_data", Some(true)),
                ("query_media", Some(true)),
                ("read_annotations", Some(true)),
                ("read_comments", Some(true)),
                ("read_note", Some(true)),
                ("reply_comment", None),
                ("resolve_comment", None),
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
