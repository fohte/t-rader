//! ノート操作の inner method 実装。
//!
//! 戦略境界の検査は [`super::fetch_note_owned_by`] が担う。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use uuid::Uuid;

use crate::entities::note;
use crate::services::graph::{GraphDef, validate_graphs};

use super::dto::{
    ListNotesParams, ListNotesResult, NoteDto, ReadNoteParams, WriteNoteParams, WriteNoteResult,
};
use super::{
    DEFAULT_NOTE_STATUS, STRATEGY_AGENT_ACTOR, StrategyServer, clamp_limit, db_error,
    ensure_strategy_exists, fetch_note_owned_by, internal_error, invalid_params,
};

/// 検証済みの `graphs` を `note.graphs_json` へ入れる JSON へ変換する。
fn graphs_to_json(graphs: Vec<GraphDef>) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(graphs)
        .map_err(|e| internal_error(format!("failed to serialize graphs: {e}")))
}

fn note_to_dto(m: note::Model) -> Result<NoteDto, McpError> {
    let graphs: Vec<GraphDef> = serde_json::from_value(m.graphs_json)
        .map_err(|e| internal_error(format!("failed to deserialize note.graphs_json: {e}")))?;
    let frontmatter_json = m
        .frontmatter_json
        .as_object()
        .cloned()
        .ok_or_else(|| internal_error("note.frontmatter_json is not a JSON object"))?;
    Ok(NoteDto {
        note_id: m.id,
        strategy_id: m.strategy_id,
        title: m.title,
        body_md: m.body_md,
        frontmatter_json,
        type_tag: m.type_tag,
        status: m.status,
        created_by_kind: m.created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
        graphs,
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

        // 明示的な note_id が最優先。無ければ同一 execution_id の既存ノートを探し、
        // あればそれを更新対象にする (agent 側のリトライによる重複作成を防ぐ)。
        let effective_note_id = match params.note_id {
            Some(note_id) => Some(note_id),
            None => match execution_id.as_deref() {
                Some(exec_id) => note::Entity::find()
                    .filter(note::Column::StrategyId.eq(session_strategy_id))
                    .filter(note::Column::ExecutionId.eq(exec_id))
                    .one(&self.db)
                    .await
                    .map_err(db_error)?
                    .map(|m| m.id),
                None => None,
            },
        };

        if let Some(note_id) = effective_note_id {
            let current = fetch_note_owned_by(&self.db, note_id, session_strategy_id).await?;
            let mut active = current.clone().into_active_model();
            let mut touched = false;
            let mut new_body_md = None;
            if let Some(title) = params.title {
                let title = title.trim().to_string();
                if title.is_empty() {
                    return Err(invalid_params("title must not be empty"));
                }
                active.title = Set(title);
                touched = true;
            }
            if let Some(body) = params.body_md {
                new_body_md = Some(body.clone());
                active.body_md = Set(body);
                touched = true;
            }
            if let Some(tag) = params.type_tag {
                active.type_tag = Set(tag);
                touched = true;
            }
            if let Some(fm) = params.frontmatter_json {
                active.frontmatter_json = Set(fm.into());
                touched = true;
            }
            if let Some(graphs) = params.graphs {
                active.graphs_json = Set(graphs_to_json(graphs)?);
                touched = true;
            }
            if !touched {
                return Err(invalid_params(
                    "at least one of title / body_md / type_tag / frontmatter_json / graphs must be provided",
                ));
            }
            // 内容が変わった時点で承認/却下時点の判断根拠は失効するため、
            // 直近の status (承認/却下含む) を無条件で unread に戻す。
            active.status = Set(DEFAULT_NOTE_STATUS.to_string());
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            let txn = self.db.begin().await.map_err(db_error)?;
            active.update(&txn).await.map_err(db_error)?;
            if let Some(body_md) = new_body_md {
                crate::services::comment_anchor::reanchor_note_comments(&txn, note_id, &body_md)
                    .await
                    .map_err(db_error)?;
            }
            txn.commit().await.map_err(db_error)?;
            return Ok(WriteNoteResult {
                note_id,
                created: false,
            });
        }

        ensure_strategy_exists(&self.db, session_strategy_id).await?;
        let title = params
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid_params("title is required when creating a new note"))?
            .to_string();
        let body_md = params.body_md.unwrap_or_default();
        let frontmatter_json: serde_json::Value =
            params.frontmatter_json.unwrap_or_default().into();
        let graphs_json = graphs_to_json(params.graphs.unwrap_or_default())?;
        let id = Uuid::new_v4();
        let model = note::ActiveModel {
            id: Set(id),
            strategy_id: Set(session_strategy_id),
            title: Set(title),
            body_md: Set(body_md),
            frontmatter_json: Set(frontmatter_json),
            type_tag: Set(params.type_tag.flatten()),
            status: Set(DEFAULT_NOTE_STATUS.to_string()),
            trigger: Set(None),
            trigger_label: Set(None),
            created_by_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
            graphs_json: Set(graphs_json),
            execution_id: Set(execution_id),
        };
        note::Entity::insert(model)
            .exec_without_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(WriteNoteResult {
            note_id: id,
            created: true,
        })
    }

    pub(crate) async fn read_note_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        let row = fetch_note_owned_by(&self.db, params.note_id, session_strategy_id).await?;
        note_to_dto(row)
    }

    pub(crate) async fn list_notes_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        let rows = note::Entity::find()
            .filter(note::Column::StrategyId.eq(session_strategy_id))
            .order_by_desc(note::Column::UpdatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ListNotesResult {
            notes: rows
                .into_iter()
                .map(note_to_dto)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;
    use uuid::Uuid;

    use sea_orm::EntityTrait;

    use crate::entities::comment;
    use crate::services::graph::{GraphDef, GraphEdge, GraphNode, Layout};
    use crate::testing::create_test_db;

    use super::super::dto::{ListNotesParams, ReadNoteParams, WriteNoteParams, WriteNoteResult};
    use super::super::tests_common::{
        build_server, insert_strategy, normalize_comment_model, normalize_note, seed_foreign_note,
        seed_note_comment_with_anchor, set_note_status, ts_sentinel,
    };
    use super::super::{DEFAULT_NOTE_STATUS, NoteDto, STRATEGY_AGENT_ACTOR};

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
                body_md: "body".into(),
                frontmatter_json: serde_json::Map::new(),
                type_tag: Some("observation".into()),
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![],
            },
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
                    body_md: "v2".into(),
                    frontmatter_json: serde_json::Map::new(),
                    type_tag: None,
                    status: DEFAULT_NOTE_STATUS.into(),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                    graphs: vec![],
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
            .read_note_inner(strategy_a, ReadNoteParams { note_id })
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
            .list_notes_inner(strategy_a, ListNotesParams { limit: None })
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
                body_md: "[[graph:g1]]".into(),
                frontmatter_json: serde_json::Map::new(),
                type_tag: None,
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![graph],
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
            .list_notes_inner(strategy_id, ListNotesParams { limit: None })
            .await
            .expect("list");
        assert_eq!(result.notes, vec![]);
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
                    body_md: expected_body,
                    frontmatter_json: serde_json::Map::new(),
                    type_tag: None,
                    status: DEFAULT_NOTE_STATUS.into(),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                    graphs: expected_graphs,
                },
                "case {label}",
            );
        }
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
                },
            )
            .await
            .expect("read");
        assert_eq!(read.status, DEFAULT_NOTE_STATUS);
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
                body_md: "orig".into(),
                frontmatter_json: serde_json::Map::new(),
                type_tag: None,
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![graph],
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
                body_md: "v2".into(),
                frontmatter_json: serde_json::Map::new(),
                type_tag: None,
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![],
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
            .list_notes_inner(strategy_id, ListNotesParams { limit: None })
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
            .list_notes_inner(strategy_id, ListNotesParams { limit: None })
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
                },
            )
            .await
            .expect("read note a");
        let read_b = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    note_id: note_b.note_id,
                },
            )
            .await
            .expect("read note b");
        assert_eq!(
            (
                normalize_note(read_a).body_md,
                normalize_note(read_b).body_md
            ),
            ("a body updated".to_string(), "b body".to_string()),
        );
    }
}
