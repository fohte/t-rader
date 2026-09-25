//! ノート操作の inner method 実装。
//!
//! 戦略境界の検査は [`super::fetch_note_owned_by`] が担う。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::{Expr, ExprTrait, OnConflict};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use uuid::Uuid;

use crate::entities::{note, note_version};
use crate::services::change_history::Actor;
use crate::services::graph::{GraphDef, validate_graphs};
use crate::services::note_links::find_links_from_version;
use crate::services::note_versions::{
    self, AppendVersion, current_note_ids_with_status, find_current_version, find_current_versions,
    find_initial_created_by_kind, find_version_of_note,
};

use super::dto::{
    ListNotesParams, ListNotesResult, NoteDto, NoteLinkDto, ReadNoteParams, WriteNoteParams,
    WriteNoteResult,
};
use super::{
    STRATEGY_AGENT_ACTOR, StrategyServer, app_error_to_mcp, clamp_limit, db_error,
    ensure_strategy_exists, fetch_note_owned_by, internal_error, invalid_params,
};

/// 検証済みの `graphs` を JSON へ変換する。
fn graphs_to_json(graphs: Vec<GraphDef>) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(graphs)
        .map_err(|e| internal_error(format!("failed to serialize graphs: {e}")))
}

/// insert 済みの note に初版を追加し、同一トランザクションを commit する。
async fn commit_new_note(
    txn: sea_orm::DatabaseTransaction,
    id: Uuid,
    content: AppendVersion,
) -> Result<WriteNoteResult, McpError> {
    note_versions::append_version(&txn, id, content)
        .await
        .map_err(app_error_to_mcp)?;
    txn.commit().await.map_err(db_error)?;
    Ok(WriteNoteResult {
        note_id: id,
        created: true,
    })
}

/// 新規ノートのメタデータと初版を組み立てる。
fn build_new_note_model(
    session_strategy_id: Uuid,
    execution_id: Option<String>,
    params: WriteNoteParams,
) -> Result<(Uuid, note::ActiveModel, AppendVersion), McpError> {
    let title = params
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid_params("title is required when creating a new note"))?
        .to_string();
    let body_md = params.body_md.unwrap_or_default();
    let frontmatter_json: serde_json::Value = params.frontmatter_json.unwrap_or_default().into();
    let graphs_json = graphs_to_json(params.graphs.unwrap_or_default())?;
    let id = Uuid::new_v4();
    Ok((
        id,
        note::ActiveModel {
            id: Set(id),
            strategy_id: Set(Some(session_strategy_id)),
            type_tag: Set(params.type_tag.flatten()),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(execution_id.clone()),
        },
        AppendVersion {
            title,
            body_md,
            frontmatter_json,
            graphs_json,
            created_by_kind: STRATEGY_AGENT_ACTOR.to_string(),
            execution_id,
            change_reason: None,
            change_diff: None,
            actor: Actor::Llm {
                label: STRATEGY_AGENT_ACTOR,
            },
        },
    ))
}

/// note_version.status の CHECK 制約と一致させる。
const ALLOWED_NOTE_STATUS: [&str; 3] = ["approved", "unread", "rejected"];

/// `m.strategy_id` は呼び出し元が `session_strategy_id` で絞り込んだ行から来るため
/// 必ず `Some` になるはずだが、不変条件が壊れた場合に別 strategy の id を誤って
/// 返さないよう fail-loud にする。
fn note_to_dto(
    m: note::Model,
    version: note_version::Model,
    created_by_kind: String,
    include_body: bool,
) -> Result<NoteDto, McpError> {
    let strategy_id = m.strategy_id.ok_or_else(|| {
        internal_error(format!(
            "note {} has no strategy_id despite session scoping",
            m.id
        ))
    })?;
    let graphs: Vec<GraphDef> = serde_json::from_value(version.graphs_json).map_err(|e| {
        internal_error(format!(
            "failed to deserialize note_version.graphs_json: {e}"
        ))
    })?;
    let frontmatter_json = version
        .frontmatter_json
        .as_object()
        .cloned()
        .ok_or_else(|| internal_error("note_version.frontmatter_json is not a JSON object"))?;
    Ok(NoteDto {
        note_id: m.id,
        strategy_id,
        title: version.title,
        body_md: include_body.then_some(version.body_md),
        frontmatter_json,
        type_tag: m.type_tag,
        status: version.status,
        created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
        graphs,
        links: None,
    })
}

impl StrategyServer {
    pub(crate) async fn write_note_inner(
        &self,
        session_strategy_id: Uuid,
        execution_id: Option<String>,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        if let Some(graphs) = params.graphs.as_ref() {
            validate_graphs(graphs).map_err(|e| invalid_params(e.to_string()))?;
        }

        let effective_note_id = match params.note_id {
            Some(note_id) => Some(note_id),
            None => match execution_id.as_deref() {
                Some(exec_id) => {
                    self.find_note_by_execution_id(session_strategy_id, exec_id)
                        .await?
                }
                None => None,
            },
        };

        if let Some(note_id) = effective_note_id {
            return self
                .update_note(session_strategy_id, note_id, execution_id, params)
                .await;
        }

        ensure_strategy_exists(&self.db, session_strategy_id).await?;

        let Some(exec_id) = execution_id else {
            return self.insert_note(session_strategy_id, None, params).await;
        };

        // execution_id 付きの create は並行呼び出しで UNIQUE 違反になりうるため
        // ON CONFLICT DO NOTHING で挿入を試み、負けた場合は勝者の行を更新対象にフォールバックする。
        let params_for_fallback = params.clone();
        match self
            .insert_note_or_conflict(session_strategy_id, exec_id.clone(), params)
            .await?
        {
            Some(result) => Ok(result),
            None => {
                let note_id = self
                    .find_note_by_execution_id(session_strategy_id, &exec_id)
                    .await?
                    .ok_or_else(|| {
                        db_error(sea_orm::DbErr::Custom(
                            "note disappeared between ON CONFLICT and SELECT".into(),
                        ))
                    })?;
                self.update_note(
                    session_strategy_id,
                    note_id,
                    Some(exec_id),
                    params_for_fallback,
                )
                .await
            }
        }
    }

    async fn find_note_by_execution_id(
        &self,
        session_strategy_id: Uuid,
        execution_id: &str,
    ) -> Result<Option<Uuid>, McpError> {
        note::Entity::find()
            .filter(note::Column::StrategyId.eq(session_strategy_id))
            .filter(note::Column::ExecutionId.eq(execution_id))
            .one(&self.db)
            .await
            .map_err(db_error)
            .map(|m| m.map(|m| m.id))
    }

    /// 既存ノートへ `params` の指定フィールドのみを部分適用する。`write_note_inner` の
    /// 「明示 note_id」経路と「execution_id の ON CONFLICT 敗北」経路の両方から呼ばれる。
    async fn update_note(
        &self,
        session_strategy_id: Uuid,
        note_id: Uuid,
        execution_id: Option<String>,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        let current = fetch_note_owned_by(&self.db, note_id, session_strategy_id).await?;
        let current_version = find_current_version(&self.db, note_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| internal_error(format!("note {note_id} has no current version")))?;
        let mut active = current.clone().into_active_model();
        let mut touched = false;
        let mut version_changed = false;
        let mut title = current_version.title.clone();
        let mut body_md = current_version.body_md.clone();
        let mut frontmatter_json = current_version.frontmatter_json.clone();
        let mut graphs_json = current_version.graphs_json.clone();
        if let Some(requested_title) = params.title {
            let updated_title = requested_title.trim().to_string();
            if updated_title.is_empty() {
                return Err(invalid_params("title must not be empty"));
            }
            title = updated_title;
            touched = true;
            version_changed = true;
        }
        if let Some(body) = params.body_md {
            body_md = body;
            touched = true;
            version_changed = true;
        }
        if let Some(tag) = params.type_tag {
            active.type_tag = Set(tag);
            touched = true;
        }
        if let Some(fm) = params.frontmatter_json {
            frontmatter_json = fm.into();
            touched = true;
            version_changed = true;
        }
        if let Some(graphs) = params.graphs {
            graphs_json = graphs_to_json(graphs)?;
            touched = true;
            version_changed = true;
        }
        if !touched {
            return Err(invalid_params(
                "at least one of title / body_md / type_tag / frontmatter_json / graphs must be provided",
            ));
        }
        active.updated_at = if version_changed {
            NotSet
        } else {
            Set(chrono::Utc::now().fixed_offset())
        };
        let txn = self.db.begin().await.map_err(db_error)?;
        active.update(&txn).await.map_err(db_error)?;
        if version_changed {
            note_versions::append_version(
                &txn,
                note_id,
                AppendVersion {
                    title,
                    body_md,
                    frontmatter_json,
                    graphs_json,
                    created_by_kind: STRATEGY_AGENT_ACTOR.to_string(),
                    execution_id,
                    change_reason: None,
                    change_diff: None,
                    actor: Actor::Llm {
                        label: STRATEGY_AGENT_ACTOR,
                    },
                },
            )
            .await
            .map_err(app_error_to_mcp)?;
        }
        txn.commit().await.map_err(db_error)?;
        Ok(WriteNoteResult {
            note_id,
            created: false,
        })
    }

    async fn insert_note(
        &self,
        session_strategy_id: Uuid,
        execution_id: Option<String>,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        let (id, model, content) = build_new_note_model(session_strategy_id, execution_id, params)?;
        let txn = self.db.begin().await.map_err(db_error)?;
        note::Entity::insert(model)
            .exec_without_returning(&txn)
            .await
            .map_err(db_error)?;
        commit_new_note(txn, id, content).await
    }

    /// `execution_id` 付きの新規作成を `ON CONFLICT (strategy_id, execution_id) DO NOTHING` で
    /// 試みる。対象インデックスは部分インデックス (`execution_id IS NOT NULL` のみ) なので、
    /// `target_and_where` で ON CONFLICT 側にも同じ述語を明示しないと Postgres がこのインデックスを
    /// arbiter に選べない (述語が一致しないと "no unique or exclusion constraint" エラーになる)。
    /// 衝突時 (`Ok(None)`) は呼び出し側が SELECT して更新にフォールバックすること。
    async fn insert_note_or_conflict(
        &self,
        session_strategy_id: Uuid,
        execution_id: String,
        params: WriteNoteParams,
    ) -> Result<Option<WriteNoteResult>, McpError> {
        let (id, model, content) =
            build_new_note_model(session_strategy_id, Some(execution_id), params)?;
        let txn = self.db.begin().await.map_err(db_error)?;
        let insert_result = note::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([note::Column::StrategyId, note::Column::ExecutionId])
                    .target_and_where(Expr::col(note::Column::ExecutionId).is_not_null())
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning(&txn)
            .await;
        match insert_result {
            Ok(_) => commit_new_note(txn, id, content).await.map(Some),
            // ON CONFLICT DO NOTHING で skip されたとき、SeaORM 2.0 では
            // `exec_with_returning` は `RecordNotFound` を返す (RETURNING 行が空のため)。
            // 念のため `RecordNotInserted` も同じパスで扱う (interests.rs の add_interest_inner と同様)。
            Err(sea_orm::DbErr::RecordNotInserted | sea_orm::DbErr::RecordNotFound(_)) => {
                txn.rollback().await.map_err(db_error)?;
                Ok(None)
            }
            Err(err) => Err(db_error(err)),
        }
    }

    pub(crate) async fn read_note_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        let row = fetch_note_owned_by(&self.db, params.note_id, session_strategy_id).await?;
        let version = find_version_of_note(&self.db, params.note_id, params.version_id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| match params.version_id {
                Some(version_id) => invalid_params(format!(
                    "version_id {version_id} does not belong to note {}",
                    params.note_id
                )),
                None => internal_error(format!("note {} has no current version", params.note_id)),
            })?;
        let created_by_kind = find_initial_created_by_kind(&self.db, &[params.note_id])
            .await
            .map_err(db_error)?
            .remove(&params.note_id)
            .ok_or_else(|| {
                internal_error(format!("note {} has no initial version", params.note_id))
            })?;
        let links = find_links_from_version(&self.db, version.id)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(|link| NoteLinkDto {
                to_note_id: link.to_note_id,
                to_version_id: link.to_version_id,
            })
            .collect();
        let mut dto = note_to_dto(row, version, created_by_kind, true)?;
        dto.links = Some(links);
        Ok(dto)
    }

    pub(crate) async fn list_notes_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        if let Some(status) = params.status.as_deref()
            && !ALLOWED_NOTE_STATUS.contains(&status)
        {
            return Err(invalid_params(format!(
                "invalid status: {status} (expected one of {ALLOWED_NOTE_STATUS:?})"
            )));
        }
        let include_body = params.include_body.unwrap_or(true);

        let mut query =
            note::Entity::find().filter(note::Column::StrategyId.eq(session_strategy_id));
        if let Some(status) = params.status {
            query =
                query.filter(note::Column::Id.in_subquery(current_note_ids_with_status(&status)));
        }
        if let Some(updated_after) = params.updated_after {
            query = query.filter(note::Column::UpdatedAt.gte(updated_after));
        }
        let rows = query
            .order_by_desc(note::Column::UpdatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        let versions =
            find_current_versions(&self.db, &rows.iter().map(|row| row.id).collect::<Vec<_>>())
                .await
                .map_err(db_error)?;
        let creators = find_initial_created_by_kind(
            &self.db,
            &rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        )
        .await
        .map_err(db_error)?;
        let notes = rows
            .into_iter()
            .map(|row| {
                let version = versions.get(&row.id).cloned().ok_or_else(|| {
                    internal_error(format!("note {} has no current version", row.id))
                })?;
                let created_by_kind = creators.get(&row.id).cloned().ok_or_else(|| {
                    internal_error(format!("note {} has no initial version", row.id))
                })?;
                note_to_dto(row, version, created_by_kind, include_body)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ListNotesResult { notes })
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;
    use uuid::Uuid;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    use crate::entities::{comment, note_ref, note_version};
    use crate::services::graph::{GraphDef, GraphEdge, GraphNode, Layout};
    use crate::services::note_versions::find_current_version;
    use crate::testing::create_test_db;

    use super::super::STRATEGY_AGENT_ACTOR;
    use super::super::dto::{
        ListNotesParams, NoteDto, ReadNoteParams, WriteNoteParams, WriteNoteResult,
    };
    use super::super::tests_common::{
        build_server, insert_strategy, normalize_comment_model, normalize_note, seed_foreign_note,
        seed_note_comment_with_anchor, set_note_status, set_note_updated_at, ts_sentinel,
    };

    const INVALID_NOTE_BODY: &str = "[[bogus:one]] [[bare-demo]]";
    const INVALID_BODY_TOKEN_ERROR: &str = concat!(
        "ノートのトークンに問題があります:\n",
        "- 本文のトークン \"[[bogus:one]]\": 未知の prefix `bogus` です\n",
        "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
        "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[sector:<id>]]`, `[[theme:<id>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 4 種のみ使用できます。",
    );
    const INVALID_NOTE_TOKEN_ERROR: &str = concat!(
        "ノートのトークンに問題があります:\n",
        "- 本文のトークン \"[[bogus:one]]\": 未知の prefix `bogus` です\n",
        "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
        "- graphs[0].nodes[0].ref の値 \"[[foo:bar]]\": 未知の prefix `foo` です; 図ノードでは stock / indicator / sector / theme の参照だけを使用できます\n",
        "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[sector:<id>]]`, `[[theme:<id>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 4 種のみ使用できます。",
    );

    fn test_node(id: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: id.to_string(),
            r#ref: None,
            value: None,
            cite: None,
            parent: None,
            x: None,
            y: None,
        }
    }

    /// `a` -> `b` の 1 edge を持つ有効な graph。
    fn sample_graph(id: &str) -> GraphDef {
        GraphDef {
            id: id.to_string(),
            layout: Layout::Flow,
            title: None,
            nodes: vec![test_node("a"), test_node("b")],
            edges: vec![GraphEdge {
                source: "a".to_string(),
                target: "b".to_string(),
                label: None,
                value: None,
                cite: None,
            }],
        }
    }

    /// `edges[0].target` が `nodes` に存在しない、検証で弾かれるべき graph。
    fn invalid_graph(id: &str) -> GraphDef {
        GraphDef {
            id: id.to_string(),
            layout: Layout::Flow,
            title: None,
            nodes: vec![test_node("a")],
            edges: vec![GraphEdge {
                source: "a".to_string(),
                target: "does-not-exist".to_string(),
                label: None,
                value: None,
                cite: None,
            }],
        }
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_creates_then_read_note_returns_it(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let written = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("first note".into()),
                    body_md: Some("body".into()),
                    type_tag: Some(Some("observation".into())),
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("write_note");
        assert!(written.created);

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: written.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read_note");

        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: written.note_id,
                strategy_id,
                title: "first note".into(),
                body_md: Some("body".into()),
                frontmatter_json: serde_json::Map::new(),
                type_tag: Some("observation".into()),
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![],
                links: Some(vec![]),
            },
        );
    }

    async fn note_refs_of(
        db: &sea_orm::DatabaseConnection,
        note_id: Uuid,
    ) -> Vec<(String, String)> {
        let mut refs = note_ref::Entity::find()
            .filter(note_ref::Column::NoteId.eq(note_id))
            .all(db)
            .await
            .expect("query note_ref");
        refs.sort_by(|a, b| (&a.ref_kind, &a.ref_id).cmp(&(&b.ref_kind, &b.ref_id)));
        refs.into_iter().map(|r| (r.ref_kind, r.ref_id)).collect()
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_creates_note_ref_from_body_and_graphs(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let mut graph = sample_graph("g1");
        graph.nodes[0].r#ref = Some("stock:7203".into());

        let written = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note with refs".into()),
                    body_md: Some("mentions [[theme:weak-jpy]]".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![graph]),
                },
            )
            .await
            .expect("write_note");

        assert_eq!(
            note_refs_of(&db, written.note_id).await,
            vec![
                ("stock".to_string(), "7203".to_string()),
                ("theme".to_string(), "weak-jpy".to_string()),
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_update_resyncs_note_refs(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note".into()),
                    body_md: Some("mentions [[stock:7203]]".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create");
        assert_eq!(
            note_refs_of(&db, created.note_id).await,
            vec![("stock".to_string(), "7203".to_string())],
        );

        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some("now mentions [[indicator:USDJPY]]".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("update");

        assert_eq!(
            note_refs_of(&db, created.note_id).await,
            vec![("indicator".to_string(), "USDJPY".to_string())],
        );
    }

    // 更新前の status (unread / rejected) ごとにケースを列挙する。
    // rstest #[case] は sqlx::test の pool 注入と組み合わせ難いため for ループで列挙する (backend/src/handlers/hypotheses.rs:450 と同様)。
    #[sqlx::test(migrations = false)]
    async fn write_note_updates_existing_and_resets_status_to_unread(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());

        for (label, initial_status) in [("already_unread", None), ("rejected", Some("rejected"))] {
            let created = server
                .write_note_inner(
                    strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: None,
                        title: Some("original".into()),
                        body_md: Some("v1".into()),
                        type_tag: None,
                        frontmatter_json: None,
                        graphs: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("case {label}: create failed: {e}"));
            if let Some(status) = initial_status {
                set_note_status(&db, created.note_id, status).await;
            }

            let updated = server
                .write_note_inner(
                    strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: Some(created.note_id),
                        title: None,
                        body_md: Some("v2".into()),
                        type_tag: None,
                        frontmatter_json: None,
                        graphs: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("case {label}: update failed: {e}"));
            assert_eq!(
                updated,
                WriteNoteResult {
                    note_id: created.note_id,
                    created: false,
                },
                "case {label}",
            );

            let read = server
                .read_note_inner(
                    strategy_id,
                    ReadNoteParams {
                        note_id: created.note_id,
                        version_id: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("case {label}: read failed: {e}"));
            assert_eq!(
                normalize_note(read),
                NoteDto {
                    note_id: created.note_id,
                    strategy_id,
                    title: "original".into(),
                    body_md: Some("v2".into()),
                    frontmatter_json: serde_json::Map::new(),
                    type_tag: None,
                    status: "unread".into(),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                    graphs: vec![],
                    links: Some(vec![]),
                },
                "case {label}",
            );
        }
    }

    /// `type_tag: Some(None)` (JSON で `"type_tag": null`) は既存タグの NULL クリアとして扱う
    #[sqlx::test(migrations = false)]
    async fn write_note_clears_type_tag_with_explicit_null(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("t".into()),
                    body_md: None,
                    type_tag: Some(Some("observation".into())),
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create");

        // タグを明示的に null へ更新
        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: None,
                    type_tag: Some(None),
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("clear");

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: created.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read");
        assert_eq!(read.type_tag, None);
    }

    /// `Option<Option<String>>` のシリアライズ意味論を pin する。
    /// フィールド省略 → `None` (touch しない)、`null` 明示 → `Some(None)` (NULL クリア)、値あり → `Some(Some(v))`。
    #[test]
    fn write_note_params_type_tag_deserialization() {
        fn parse(json: &str) -> Option<Option<String>> {
            serde_json::from_str::<WriteNoteParams>(json)
                .expect("parse")
                .type_tag
        }
        assert_eq!(
            (
                parse("{}"),
                parse(r#"{"type_tag":null}"#),
                parse(r#"{"type_tag":"observation"}"#),
            ),
            (None, Some(None), Some(Some("observation".into()))),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_cross_strategy_update(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        let err = server
            .write_note_inner(
                strategy_a,
                None,
                WriteNoteParams {
                    note_id: Some(note_id),
                    title: None,
                    body_md: Some("hijack".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect_err("cross-strategy update expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_note_rejects_cross_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        let err = server
            .read_note_inner(
                strategy_a,
                ReadNoteParams {
                    note_id,
                    version_id: None,
                },
            )
            .await
            .expect_err("cross-strategy read expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_filters_by_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db);

        for (sid, title) in [(strategy_a, "a1"), (strategy_a, "a2"), (strategy_b, "b1")] {
            server
                .write_note_inner(
                    sid,
                    None,
                    WriteNoteParams {
                        note_id: None,
                        title: Some(title.into()),
                        body_md: None,
                        type_tag: None,
                        frontmatter_json: None,
                        graphs: None,
                    },
                )
                .await
                .expect("write");
        }

        let result = server
            .list_notes_inner(strategy_a, ListNotesParams::default())
            .await
            .expect("list");
        // 戦略 B のノートは含まれず、戦略 A の 2 件のみが新しい順に並ぶ
        let titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        let strategies: Vec<Uuid> = result.notes.iter().map(|n| n.strategy_id).collect();
        assert_eq!(
            (titles, strategies),
            (vec!["a2", "a1"], vec![strategy_a, strategy_a]),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_filters_by_status(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());

        for (title, status) in [
            ("u", None),
            ("a", Some("approved")),
            ("r", Some("rejected")),
        ] {
            let created = server
                .write_note_inner(
                    strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: None,
                        title: Some(title.into()),
                        body_md: None,
                        type_tag: None,
                        frontmatter_json: None,
                        graphs: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("write {title} failed: {e}"));
            if let Some(status) = status {
                set_note_status(&db, created.note_id, status).await;
            }
        }

        let result = server
            .list_notes_inner(
                strategy_id,
                ListNotesParams {
                    status: Some("approved".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("list");
        let titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, vec!["a"]);
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_rejects_invalid_status(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);

        let err = server
            .list_notes_inner(
                strategy_id,
                ListNotesParams {
                    status: Some("bogus".into()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("invalid status expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_filters_by_updated_after(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());

        let old = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("old".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("write old");
        let new = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("new".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("write new");

        let now = chrono::Utc::now().fixed_offset();
        set_note_updated_at(&db, old.note_id, now - chrono::Duration::days(2)).await;
        set_note_updated_at(&db, new.note_id, now - chrono::Duration::hours(1)).await;

        let result = server
            .list_notes_inner(
                strategy_id,
                ListNotesParams {
                    updated_after: Some(now - chrono::Duration::days(1)),
                    ..Default::default()
                },
            )
            .await
            .expect("list");
        let titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, vec!["new"]);
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_include_body_false_omits_body(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);

        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("t".into()),
                    body_md: Some("secret".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("write");

        let result = server
            .list_notes_inner(
                strategy_id,
                ListNotesParams {
                    include_body: Some(false),
                    ..Default::default()
                },
            )
            .await
            .expect("list");
        let bodies: Vec<Option<&str>> = result.notes.iter().map(|n| n.body_md.as_deref()).collect();
        assert_eq!(bodies, vec![None]);
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_creates_with_graphs_then_read_note_returns_them(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let graph = sample_graph("g1");
        let written = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note with graph".into()),
                    body_md: Some("[[graph:g1]]".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![graph.clone()]),
                },
            )
            .await
            .expect("write_note");

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: written.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read_note");

        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: written.note_id,
                strategy_id,
                title: "note with graph".into(),
                body_md: Some("[[graph:g1]]".into()),
                frontmatter_json: serde_json::Map::new(),
                type_tag: None,
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![graph],
                links: Some(vec![]),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_invalid_graph_and_does_not_create_note(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let err = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("broken".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![invalid_graph("g1")]),
                },
            )
            .await
            .expect_err("invalid graph expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);

        let result = server
            .list_notes_inner(strategy_id, ListNotesParams::default())
            .await
            .expect("list");
        assert_eq!(result.notes, vec![]);
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_invalid_body_tokens_without_creating_note(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);
        let mut graph = sample_graph("g1");
        graph.nodes[0].r#ref = Some("foo:bar".into());

        let err = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("token validation".into()),
                    body_md: Some(INVALID_NOTE_BODY.into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![graph]),
                },
            )
            .await
            .expect_err("invalid body tokens should be rejected");
        let notes = server
            .list_notes_inner(strategy_id, ListNotesParams::default())
            .await
            .expect("list notes");

        assert_eq!(
            (err.code, err.message.to_string(), notes.notes.len()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                INVALID_NOTE_TOKEN_ERROR.to_string(),
                0,
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_invalid_body_tokens_and_keeps_existing_note_unchanged(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);
        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("token validation".into()),
                    body_md: Some("original".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create note");

        let err = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some(INVALID_NOTE_BODY.into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect_err("invalid body tokens should be rejected");
        let note = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: created.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note");

        assert_eq!(
            (err.code, err.message.to_string(), note.body_md),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                INVALID_BODY_TOKEN_ERROR.to_string(),
                Some("original".to_string()),
            ),
        );
    }

    // body_md と graphs は独立に部分更新できる: 片方だけ送るともう片方は無傷。
    #[sqlx::test(migrations = false)]
    async fn write_note_update_graphs_and_body_are_independent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        for (label, update_body, update_graphs, expected_body, expected_graphs) in [
            (
                "omit_graphs_leaves_them_unchanged",
                Some("v2".to_string()),
                None,
                "v2".to_string(),
                vec![sample_graph("g1")],
            ),
            (
                "graphs_only_replaces_array_leaves_body_untouched",
                None,
                Some(vec![sample_graph("g2")]),
                "orig".to_string(),
                vec![sample_graph("g2")],
            ),
        ] {
            let created = server
                .write_note_inner(
                    strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: None,
                        title: Some("t".into()),
                        body_md: Some("orig".into()),
                        type_tag: None,
                        frontmatter_json: None,
                        graphs: Some(vec![sample_graph("g1")]),
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("case {label}: create failed: {e}"));

            server
                .write_note_inner(
                    strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: Some(created.note_id),
                        title: None,
                        body_md: update_body,
                        type_tag: None,
                        frontmatter_json: None,
                        graphs: update_graphs,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("case {label}: update failed: {e}"));

            let read = server
                .read_note_inner(
                    strategy_id,
                    ReadNoteParams {
                        note_id: created.note_id,
                        version_id: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("case {label}: read failed: {e}"));
            assert_eq!(
                normalize_note(read),
                NoteDto {
                    note_id: created.note_id,
                    strategy_id,
                    title: "t".into(),
                    body_md: Some(expected_body),
                    frontmatter_json: serde_json::Map::new(),
                    type_tag: None,
                    status: "unread".into(),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                    graphs: expected_graphs,
                    links: Some(vec![]),
                },
                "case {label}",
            );
        }
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_graphs_only_update_keeps_legacy_body_tokens(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("token validation".into()),
                    body_md: Some("original".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![sample_graph("g1")]),
                },
            )
            .await
            .expect("create note");

        let current_version = find_current_version(&db, created.note_id)
            .await
            .expect("load current version")
            .expect("current version exists");
        note_version::ActiveModel {
            id: Set(current_version.id),
            body_md: Set("[[legacy:token]] [[stock:demo-code]]".into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("seed legacy body");

        let updated = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![sample_graph("g2")]),
                },
            )
            .await
            .expect("graphs-only update");
        let note = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: created.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note");

        assert_eq!(
            (
                updated.created,
                note.body_md,
                note.graphs
                    .iter()
                    .map(|graph| graph.id.clone())
                    .collect::<Vec<_>>(),
                note_refs_of(&db, created.note_id).await,
            ),
            (
                false,
                Some("[[legacy:token]] [[stock:demo-code]]".to_string()),
                vec!["g2".to_string()],
                vec![("stock".to_string(), "demo-code".to_string())],
            ),
        );
    }

    /// 図のみを更新した場合も、他フィールド更新と同様に status が unread へ戻る。
    #[sqlx::test(migrations = false)]
    async fn write_note_update_with_graphs_only_resets_status_to_unread(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("t".into()),
                    body_md: Some("orig".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![sample_graph("g1")]),
                },
            )
            .await
            .expect("create");
        set_note_status(&db, created.note_id, "approved").await;

        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![sample_graph("g2")]),
                },
            )
            .await
            .expect("update graphs only");

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: created.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read");
        assert_eq!(read.status, "unread");
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_invalid_graph_on_update_and_leaves_note_unchanged(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let graph = sample_graph("g1");
        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("t".into()),
                    body_md: Some("orig".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![graph.clone()]),
                },
            )
            .await
            .expect("create");

        let err = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some("hijacked".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: Some(vec![invalid_graph("g2")]),
                },
            )
            .await
            .expect_err("invalid graph on update expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: created.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read");
        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: created.note_id,
                strategy_id,
                title: "t".into(),
                body_md: Some("orig".into()),
                frontmatter_json: serde_json::Map::new(),
                type_tag: None,
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![graph],
                links: Some(vec![]),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_updating_body_md_reanchors_comment_when_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note".into()),
                    body_md: Some(
                        indoc::indoc! {"
                        line one
                        line two
                        line three"}
                        .into(),
                    ),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create");
        let comment_id = seed_note_comment_with_anchor(&db, created.note_id, "line two").await;

        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some(
                        indoc::indoc! {"
                        prefix
                        line one
                        line two
                        line three"}
                        .into(),
                    ),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("update");

        let updated_comment = comment::Entity::find_by_id(comment_id)
            .one(&db)
            .await
            .expect("query")
            .expect("comment exists");
        assert_eq!(
            normalize_comment_model(updated_comment),
            comment::Model {
                id: comment_id,
                target_kind: "note".into(),
                target_id: created.note_id,
                parent_id: None,
                body: "please fix".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                created_at: ts_sentinel(),
                resolved: false,
                anchor_text: Some("line two".into()),
                start_line: Some(3),
                end_line: Some(3),
                drifted: false,
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_updating_body_md_marks_drifted_when_anchor_missing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let created = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note".into()),
                    body_md: Some(
                        indoc::indoc! {"
                        line one
                        line two
                        line three"}
                        .into(),
                    ),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create");
        let comment_id = seed_note_comment_with_anchor(&db, created.note_id, "line two").await;

        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some("completely rewritten".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("update");

        let updated_comment = comment::Entity::find_by_id(comment_id)
            .one(&db)
            .await
            .expect("query")
            .expect("comment exists");
        assert_eq!(
            normalize_comment_model(updated_comment),
            comment::Model {
                id: comment_id,
                target_kind: "note".into(),
                target_id: created.note_id,
                parent_id: None,
                body: "please fix".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                created_at: ts_sentinel(),
                resolved: false,
                anchor_text: Some("line two".into()),
                start_line: None,
                end_line: None,
                drifted: true,
            },
        );
    }

    /// 同一 execution_id での 2 回目の create 呼び出し (note_id 省略) は 1 回目のノートを
    /// 更新する (agent 側のリトライによる重複作成を防ぐ)。
    #[sqlx::test(migrations = false)]
    async fn write_note_with_same_execution_id_collapses_onto_single_note(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let first = server
            .write_note_inner(
                strategy_id,
                Some("exec-1".into()),
                WriteNoteParams {
                    note_id: None,
                    title: Some("first".into()),
                    body_md: Some("v1".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("first write");
        assert!(first.created);

        let second = server
            .write_note_inner(
                strategy_id,
                Some("exec-1".into()),
                WriteNoteParams {
                    note_id: None,
                    title: Some("second".into()),
                    body_md: Some("v2".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("second write");
        assert_eq!(
            second,
            WriteNoteResult {
                note_id: first.note_id,
                created: false,
            },
        );

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: first.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read");
        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: first.note_id,
                strategy_id,
                title: "second".into(),
                body_md: Some("v2".into()),
                frontmatter_json: serde_json::Map::new(),
                type_tag: None,
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![],
                links: Some(vec![]),
            },
        );
    }

    /// execution_id が異なれば別ノートとして作成される。
    #[sqlx::test(migrations = false)]
    async fn write_note_with_different_execution_ids_creates_distinct_notes(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let first = server
            .write_note_inner(
                strategy_id,
                Some("exec-1".into()),
                WriteNoteParams {
                    note_id: None,
                    title: Some("a".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("first write");
        let second = server
            .write_note_inner(
                strategy_id,
                Some("exec-2".into()),
                WriteNoteParams {
                    note_id: None,
                    title: Some("b".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("second write");

        let result = server
            .list_notes_inner(strategy_id, ListNotesParams::default())
            .await
            .expect("list");
        let mut titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        titles.sort_unstable();
        assert_eq!(
            (
                titles,
                first.created,
                second.created,
                first.note_id == second.note_id
            ),
            (vec!["a", "b"], true, true, false),
        );
    }

    /// execution_id を省略した場合 (ヘッダ非対応クライアント互換) は従来通り毎回別ノートを作成する。
    #[sqlx::test(migrations = false)]
    async fn write_note_without_execution_id_creates_distinct_notes(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let first = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("a".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("first write");
        let second = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("b".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("second write");

        let result = server
            .list_notes_inner(strategy_id, ListNotesParams::default())
            .await
            .expect("list");
        let mut titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        titles.sort_unstable();
        assert_eq!(
            (
                titles,
                first.created,
                second.created,
                first.note_id == second.note_id
            ),
            (vec!["a", "b"], true, true, false),
        );
    }

    /// 明示的な note_id は execution_id によるノート解決より常に優先される。
    #[sqlx::test(migrations = false)]
    async fn write_note_explicit_note_id_wins_over_execution_id_lookup(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        // note B: execution_id "exec-1" に紐づく既存ノート
        let note_b = server
            .write_note_inner(
                strategy_id,
                Some("exec-1".into()),
                WriteNoteParams {
                    note_id: None,
                    title: Some("note b".into()),
                    body_md: Some("b body".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create note b");

        // note A: execution_id を持たない別ノート
        let note_a = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note a".into()),
                    body_md: Some("a body".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("create note a");

        // note_id: Some(note_a) かつ execution_id: Some("exec-1") (note b を指す) →
        // 明示的な note_id が優先され note a が更新される。
        let updated = server
            .write_note_inner(
                strategy_id,
                Some("exec-1".into()),
                WriteNoteParams {
                    note_id: Some(note_a.note_id),
                    title: None,
                    body_md: Some("a body updated".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("update note a");
        assert_eq!(
            updated,
            WriteNoteResult {
                note_id: note_a.note_id,
                created: false,
            },
        );

        let read_a = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: note_a.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note a");
        let read_b = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: note_b.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note b");
        assert_eq!(
            (
                normalize_note(read_a).body_md,
                normalize_note(read_b).body_md
            ),
            (
                Some("a body updated".to_string()),
                Some("b body".to_string())
            ),
        );
    }

    /// `insert_note_or_conflict` は同一 (strategy_id, execution_id) の行が既に存在するとき、
    /// パーシャルユニークインデックスへの生の制約違反エラーを投げず `Ok(None)` を返す。
    /// 作成経路を通さず実行 ID 付きの既存ノートを用意し、並行作成の先勝ちを模している。
    #[sqlx::test(migrations = false)]
    async fn insert_note_or_conflict_returns_none_when_execution_id_already_taken(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        crate::testing::insert_test_note_with_execution_id(
            &db,
            strategy_id,
            "winner",
            "winner body",
            "exec-1",
        )
        .await;

        let result = server
            .insert_note_or_conflict(
                strategy_id,
                "exec-1".into(),
                WriteNoteParams {
                    note_id: None,
                    title: Some("loser".into()),
                    body_md: Some("loser body".into()),
                    type_tag: None,
                    frontmatter_json: None,
                    graphs: None,
                },
            )
            .await
            .expect("ON CONFLICT DO NOTHING must not raise a raw constraint-violation error");
        assert_eq!(result, None);
    }
}
