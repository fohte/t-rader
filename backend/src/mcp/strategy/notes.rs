//! ノート操作の inner method 実装。
//!
//! 戦略境界の検査は [`super::fetch_note_owned_by`] が担う。

use crate::services::graph::GraphDef;
use crate::services::note_kinds;
use crate::services::note_links::find_links_from_version;
use crate::services::note_versions::{
    self, current_note_ids, current_note_ids_with_status, find_current_versions,
    find_initial_created_by_kind, find_version_of_note,
};
use core_application::change_history::Actor;
use core_application::note::{NoteUseCaseError, NoteWriteCommand};
use core_application::strategy_scope::StrategyScope;
use gateway_postgres::entities::{note, note_version};
use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use super::dto::{
    ListNoteKindsResult, ListNotesParams, ListNotesResult, NoteDto, NoteKindDto, NoteLinkDto,
    ReadNoteParams, WriteNoteParams, WriteNoteResult,
};
use super::{
    STRATEGY_AGENT_ACTOR, StrategyServer, app_error_to_mcp, clamp_limit, db_error,
    fetch_note_owned_by, internal_error, invalid_params,
};

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
        version_id: version.id,
        version_no: version.version_no,
        title: version.title,
        body_md: include_body.then_some(version.body_md),
        frontmatter_json,
        kind: m.kind,
        status: version.status,
        created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
        graphs,
        links: None,
    })
}

fn note_use_case_to_mcp(error: NoteUseCaseError) -> McpError {
    match error {
        NoteUseCaseError::Validation(message) => invalid_params(message),
        NoteUseCaseError::UnknownNoteKind(kind) => {
            invalid_params(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::ReferencedNoteKindNotFound(kind) => {
            internal_error(format!("note kind {kind} not found"))
        }
        NoteUseCaseError::NotFound(_) => McpError::resource_not_found("note not found", None),
        NoteUseCaseError::Forbidden(note_id) => invalid_params(format!(
            "forbidden: note {note_id} belongs to another strategy"
        )),
        other => internal_error(format!("{other}")),
    }
}

impl StrategyServer {
    pub(crate) async fn list_note_kinds_inner(&self) -> Result<ListNoteKindsResult, McpError> {
        let note_kinds = note_kinds::list(&self.db)
            .await
            .map_err(app_error_to_mcp)?
            .into_iter()
            .map(|kind| NoteKindDto {
                key: kind.key,
                display_name: kind.display_name,
                requires_approval: kind.requires_approval,
                description: kind.description,
                sort_order: kind.sort_order,
            })
            .collect();
        Ok(ListNoteKindsResult { note_kinds })
    }

    pub(crate) async fn write_note_inner(
        &self,
        scope: impl Into<StrategyScope>,
        execution_id: Option<String>,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        let scope = scope.into();
        let graphs_json = params
            .graphs
            .map(|graphs| {
                let graphs: Vec<core_domain::note_graph::GraphDef> =
                    graphs.into_iter().map(Into::into).collect();
                serde_json::to_value(graphs)
                    .map_err(|error| internal_error(format!("failed to serialize graphs: {error}")))
            })
            .transpose()?;
        let result = self
            .use_cases
            .notes
            .write(NoteWriteCommand {
                scope: Some(scope),
                strategy_id: Some(scope.id()),
                execution_id,
                note_id: params.note_id,
                title: params.title,
                body_md: params.body_md,
                frontmatter_json: params.frontmatter_json.map(serde_json::Value::Object),
                graphs_json,
                kind: params.kind,
                status: None,
                trigger: None,
                trigger_label: None,
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                change_reason: params.change_reason,
                actor: Actor::Llm {
                    label: STRATEGY_AGENT_ACTOR,
                },
                change_diff: None,
            })
            .await
            .map_err(note_use_case_to_mcp)?;
        Ok(WriteNoteResult {
            note_id: result.note_id,
            created: result.created,
        })
    }

    pub(crate) async fn read_note_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        let session_strategy_id = scope.into().id();
        let row = fetch_note_owned_by(&self.db, params.note_id, session_strategy_id).await?;
        let version = match params.version_id {
            Some(version_id) => find_version_of_note(&self.db, params.note_id, Some(version_id))
                .await
                .map_err(db_error)?
                .ok_or_else(|| {
                    invalid_params(format!(
                        "version_id {version_id} does not belong to note {}",
                        params.note_id
                    ))
                })?,
            None => note_versions::find_current_or_latest_version(&self.db, params.note_id)
                .await
                .map_err(db_error)?
                .ok_or_else(|| internal_error(format!("note {} has no version", params.note_id)))?,
        };
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
        scope: impl Into<StrategyScope>,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        let session_strategy_id = scope.into().id();
        if let Some(status) = params.status.as_deref()
            && !ALLOWED_NOTE_STATUS.contains(&status)
        {
            return Err(invalid_params(format!(
                "invalid status: {status} (expected one of {ALLOWED_NOTE_STATUS:?})"
            )));
        }
        let include_body = params.include_body.unwrap_or(true);

        let mut query = note::Entity::find()
            .filter(note::Column::StrategyId.eq(session_strategy_id))
            .filter(note::Column::Id.in_subquery(current_note_ids()));
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
    use uuid::Uuid;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    use super::super::STRATEGY_AGENT_ACTOR;
    use super::super::dto::{
        ListNotesParams, NoteDto, ReadNoteParams, WriteNoteParams, WriteNoteResult,
    };
    use super::super::tests_common::{
        build_server, current_note_version_id, insert_note_kind, insert_strategy,
        normalize_comment_model, normalize_note, seed_foreign_note,
        seed_note_version_comment_with_anchor, set_note_status, set_note_updated_at, ts_sentinel,
    };
    use crate::services::graph::{GraphDef, GraphEdge, GraphNode, Layout};
    use crate::services::note_versions::find_current_version;
    use gateway_postgres::entities::{comment, note, note_ref, note_version};

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

    #[backend_test_macros::database_test]
    async fn write_note_creates_then_read_note_returns_it(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        insert_note_kind(&db, "sample-kind", false).await;
        let server = build_server(db);

        let written = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("first note".into()),
                    body_md: Some("body".into()),
                    kind: Some(Some("sample-kind".into())),
                    frontmatter_json: None,
                    change_reason: None,
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
                version_id: Uuid::nil(),
                version_no: 1,
                title: "first note".into(),
                body_md: Some("body".into()),
                frontmatter_json: serde_json::Map::new(),
                kind: Some("sample-kind".into()),
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![],
                links: Some(vec![]),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn write_note_rejects_unknown_kind_as_invalid_params(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let error = server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("sample note".into()),
                    body_md: None,
                    kind: Some(Some("sample-kind".into())),
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect_err("unknown note kind must be rejected");
        let saved_notes = note::Entity::find().all(&db).await.unwrap();

        assert_eq!(
            (error.code, error.message.as_ref(), saved_notes.len(),),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "unknown note kind: sample-kind",
                0,
            ),
        );
    }

    async fn note_refs_of(
        db: &impl sea_orm::ConnectionTrait,
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

    #[backend_test_macros::database_test]
    async fn write_note_creates_note_ref_from_body_and_graphs(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn write_note_update_resyncs_note_refs(db: gateway_postgres::DatabaseHandle) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
    // database_test は rstest の case 引数を扱わないため、for ループで列挙する。
    #[backend_test_macros::database_test]
    async fn write_note_updates_existing_and_resets_status_to_unread(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                        kind: None,
                        frontmatter_json: None,
                        change_reason: None,
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
                        kind: None,
                        frontmatter_json: None,
                        change_reason: None,
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
                    version_id: Uuid::nil(),
                    version_no: 2,
                    title: "original".into(),
                    body_md: Some("v2".into()),
                    frontmatter_json: serde_json::Map::new(),
                    kind: None,
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

    /// `Option<Option<String>>` は省略と新規作成時の null を区別する。
    #[test]
    fn write_note_params_kind_deserialization() {
        fn parse(json: &str) -> Option<Option<String>> {
            serde_json::from_str::<WriteNoteParams>(json)
                .expect("parse")
                .kind
        }
        assert_eq!(
            (
                parse("{}"),
                parse(r#"{"kind":null}"#),
                parse(r#"{"kind":"sample-kind"}"#),
            ),
            (None, Some(None), Some(Some("sample-kind".into()))),
        );
    }

    #[backend_test_macros::database_test]
    async fn write_note_rejects_cross_strategy_update(db: gateway_postgres::DatabaseHandle) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect_err("cross-strategy update expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[backend_test_macros::database_test]
    async fn read_note_rejects_cross_strategy(db: gateway_postgres::DatabaseHandle) {
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

    #[backend_test_macros::database_test]
    async fn list_notes_filters_by_strategy(db: gateway_postgres::DatabaseHandle) {
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
                        kind: None,
                        frontmatter_json: None,
                        change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn list_notes_filters_by_status(db: gateway_postgres::DatabaseHandle) {
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
                        kind: None,
                        frontmatter_json: None,
                        change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn list_notes_rejects_invalid_status(db: gateway_postgres::DatabaseHandle) {
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

    #[backend_test_macros::database_test]
    async fn list_notes_filters_by_updated_after(db: gateway_postgres::DatabaseHandle) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn list_notes_include_body_false_omits_body(db: gateway_postgres::DatabaseHandle) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn write_note_creates_with_graphs_then_read_note_returns_them(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                version_id: Uuid::nil(),
                version_no: 1,
                title: "note with graph".into(),
                body_md: Some("[[graph:g1]]".into()),
                frontmatter_json: serde_json::Map::new(),
                kind: None,
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![graph],
                links: Some(vec![]),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn write_note_rejects_invalid_graph_and_does_not_create_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn write_note_rejects_invalid_body_tokens_without_creating_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn write_note_rejects_invalid_body_tokens_and_keeps_existing_note_unchanged(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
    #[backend_test_macros::database_test]
    async fn write_note_update_graphs_and_body_are_independent(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                        kind: None,
                        frontmatter_json: None,
                        change_reason: None,
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
                        kind: None,
                        frontmatter_json: None,
                        change_reason: None,
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
                    version_id: Uuid::nil(),
                    version_no: 2,
                    title: "t".into(),
                    body_md: Some(expected_body),
                    frontmatter_json: serde_json::Map::new(),
                    kind: None,
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

    #[backend_test_macros::database_test]
    async fn write_note_graphs_only_update_keeps_legacy_body_tokens(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
    #[backend_test_macros::database_test]
    async fn write_note_update_with_graphs_only_resets_status_to_unread(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    #[backend_test_macros::database_test]
    async fn write_note_rejects_invalid_graph_on_update_and_leaves_note_unchanged(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                version_id: Uuid::nil(),
                version_no: 1,
                title: "t".into(),
                body_md: Some("orig".into()),
                frontmatter_json: serde_json::Map::new(),
                kind: None,
                status: "unread".into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
                graphs: vec![graph],
                links: Some(vec![]),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn write_note_updating_body_md_keeps_comment_on_original_version(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("create");
        let version_id = current_note_version_id(&db, created.note_id).await;
        let comment_id = seed_note_version_comment_with_anchor(&db, version_id, "line two").await;

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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                target_kind: "note_version".into(),
                target_id: version_id,
                parent_id: None,
                body: "please fix".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                created_at: ts_sentinel(),
                resolved: false,
                anchor_text: Some("line two".into()),
                anchor_side: Some("new".into()),
                start_line: Some(2),
                end_line: Some(2),
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn write_note_updating_body_md_keeps_comment_position_on_original_version(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("create");
        let version_id = current_note_version_id(&db, created.note_id).await;
        let comment_id = seed_note_version_comment_with_anchor(&db, version_id, "line two").await;

        server
            .write_note_inner(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some("completely rewritten".into()),
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                target_kind: "note_version".into(),
                target_id: version_id,
                parent_id: None,
                body: "please fix".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                created_at: ts_sentinel(),
                resolved: false,
                anchor_text: Some("line two".into()),
                anchor_side: Some("new".into()),
                start_line: Some(2),
                end_line: Some(2),
            },
        );
    }

    /// 同一 execution_id での 2 回目の create 呼び出し (note_id 省略) は 1 回目のノートを
    /// 更新する (agent 側のリトライによる重複作成を防ぐ)。
    #[backend_test_macros::database_test]
    async fn write_note_with_same_execution_id_collapses_onto_single_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                version_id: Uuid::nil(),
                version_no: 2,
                title: "second".into(),
                body_md: Some("v2".into()),
                frontmatter_json: serde_json::Map::new(),
                kind: None,
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
    #[backend_test_macros::database_test]
    async fn write_note_with_different_execution_ids_creates_distinct_notes(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
    #[backend_test_macros::database_test]
    async fn write_note_without_execution_id_creates_distinct_notes(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
    #[backend_test_macros::database_test]
    async fn write_note_explicit_note_id_wins_over_execution_id_lookup(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
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

    /// 既存の execution_id へ書き込むと新規作成ではなく更新になる。
    #[backend_test_macros::database_test]
    async fn write_note_with_existing_execution_id_updates_the_existing_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let note_id = crate::testing::insert_test_note_with_execution_id(
            &db,
            strategy_id,
            "winner",
            "winner body",
            "exec-1",
        )
        .await;

        let result = server
            .write_note_inner(
                strategy_id,
                Some("exec-1".into()),
                WriteNoteParams {
                    note_id: None,
                    title: Some("revised title".into()),
                    body_md: Some("revised body".into()),
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .unwrap();
        let current = find_current_version(&db, note_id).await.unwrap().unwrap();
        assert_eq!(
            (result, current.title, current.body_md),
            (
                WriteNoteResult {
                    note_id,
                    created: false,
                },
                "revised title".into(),
                "revised body".into(),
            ),
        );
    }
}
