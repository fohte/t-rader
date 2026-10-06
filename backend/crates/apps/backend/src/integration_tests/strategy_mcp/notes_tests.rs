#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    use super::super::dto::{
        ListNotesParams, NoteDto, ReadNoteParams, WriteNoteParams, WriteNoteResult,
    };
    use super::super::graph_dto::{GraphDef, GraphEdge, GraphNode, Layout};
    use super::super::tests_common::{
        build_server, current_note_version_id, insert_note_kind, insert_strategy,
        normalize_comment_model, normalize_list_notes, normalize_note, seed_note,
        seed_note_version_comment_with_anchor, set_note_status, set_note_updated_at, ts_sentinel,
    };
    use super::super::{MAX_LIST_LIMIT, STRATEGY_AGENT_ACTOR};
    use crate::testing::find_current_note_version;
    use gateway_postgres::entities::{comment, note, note_ref, note_version};

    const INVALID_NOTE_BODY: &str = "[[bogus:one]] [[bare-demo]]";
    const INVALID_BODY_TOKEN_ERROR: &str = concat!(
        "ノートのトークンに問題があります:\n",
        "- 本文のトークン \"[[bogus:one]]\": 未知の prefix `bogus` です\n",
        "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
        "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[group:<axis-key>/<group-key>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`, `[[price:<id>@<date>:<field>]]`, `[[change:<id>@<start>..<end>:<field>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 3 種のみ使用できます。",
    );
    const INVALID_NOTE_TOKEN_ERROR: &str = concat!(
        "ノートのトークンに問題があります:\n",
        "- 本文のトークン \"[[bogus:one]]\": 未知の prefix `bogus` です\n",
        "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
        "- graphs[0].nodes[0].ref の値 \"[[foo:bar]]\": 未知の prefix `foo` です; 図ノードでは stock / indicator / group の参照だけを使用できます\n",
        "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[group:<axis-key>/<group-key>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`, `[[price:<id>@<date>:<field>]]`, `[[change:<id>@<start>..<end>:<field>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 3 種のみ使用できます。",
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

    struct PendingNoteFixture {
        strategy_id: Uuid,
        current_note_id: Uuid,
        pending_note_id: Uuid,
        pending_graph: GraphDef,
    }

    async fn create_pending_note_fixture(
        db: &gateway_postgres::DatabaseHandle,
        server: &super::super::StrategyServer,
    ) -> PendingNoteFixture {
        let strategy_id = insert_strategy(db, "a").await;
        insert_note_kind(db, "sample-kind", true).await;
        let current = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("current".into()),
                    body_md: Some("current body".into()),
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("write current note");

        let mut pending_graph = sample_graph("g1");
        pending_graph.nodes[0].r#ref = Some("group:graph-axis/graph-group".into());
        let pending = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("pending first".into()),
                    body_md: Some("[[theme:demo-topic]] [[group:body-axis/body-group]]".into()),
                    kind: Some(Some("sample-kind".into())),
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: Some(vec![pending_graph.clone()]),
                },
            )
            .await
            .expect("write pending note");
        server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(pending.note_id),
                    title: Some("pending latest".into()),
                    body_md: Some("[[stock:demo-code]]".into()),
                    kind: None,
                    frontmatter_json: None,
                    change_reason: Some("revision".into()),
                    graphs: None,
                },
            )
            .await
            .expect("write latest pending version");

        let sentinel = ts_sentinel();
        set_note_updated_at(db, current.note_id, sentinel + chrono::Duration::seconds(1)).await;
        set_note_updated_at(db, pending.note_id, sentinel + chrono::Duration::seconds(2)).await;

        PendingNoteFixture {
            strategy_id,
            current_note_id: current.note_id,
            pending_note_id: pending.note_id,
            pending_graph,
        }
    }

    fn expected_current_note(note_id: Uuid) -> NoteDto {
        NoteDto {
            note_id,
            version_id: Uuid::nil(),
            version_no: 1,
            title: "current".into(),
            body_md: Some("current body".into()),
            frontmatter_json: serde_json::Map::new(),
            tags: vec![],
            kind: None,
            status: "unread".into(),
            created_by_kind: STRATEGY_AGENT_ACTOR.into(),
            created_at: ts_sentinel(),
            updated_at: ts_sentinel(),
            graphs: vec![],
            links: None,
        }
    }

    fn expected_latest_pending_note(fixture: &PendingNoteFixture, status: &str) -> NoteDto {
        NoteDto {
            note_id: fixture.pending_note_id,
            version_id: Uuid::nil(),
            version_no: 2,
            title: "pending latest".into(),
            body_md: Some("[[stock:demo-code]]".into()),
            frontmatter_json: serde_json::Map::new(),
            tags: vec![],
            kind: Some("sample-kind".into()),
            status: status.into(),
            created_by_kind: STRATEGY_AGENT_ACTOR.into(),
            created_at: ts_sentinel(),
            updated_at: ts_sentinel(),
            graphs: vec![fixture.pending_graph.clone()],
            links: None,
        }
    }

    async fn set_latest_note_version_status(
        db: &gateway_postgres::DatabaseHandle,
        note_id: Uuid,
        status: &str,
    ) {
        let version = note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .order_by_desc(note_version::Column::VersionNo)
            .one(db)
            .await
            .expect("find latest note version")
            .expect("latest note version exists");
        note_version::ActiveModel {
            id: Set(version.id),
            status: Set(status.to_string()),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("set latest note version status");
    }

    #[backend_test_macros::database_test]
    async fn write_note_returns_warnings_and_saves_bare_values(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "long").await;
        insert_note_kind(&db, "sample-kind", false).await;
        let server = build_server(db);

        let written = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("first note".into()),
                    body_md: Some("close 12,345円 then -6.0%; 73足".into()),
                    kind: Some(Some("sample-kind".into())),
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("write_note");
        assert_eq!(
            written,
            WriteNoteResult {
                note_id: written.note_id,
                created: true,
                warnings: vec![
                    "価格候補の数値「12,345円」がリンク外にあります。株価であれば、銘柄・日付・項目を確認して `[[price:<id>@<date>:<field>]]` で参照してください。".into(),
                    "相対表現「-6.0%」があります。計算結果を手入力せず、対象期間の値を `[[change:<id>@<start>..<end>:<field>]]` で示してください。概念上の目安ならそのままで構いません。".into(),
                    "ローソク足の本数「73足」があります。本数ではなく、開始日と終了日で対象期間を記載してください。".into(),
                ],
            },
        );

        let read = server
            .read_note(
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
                version_id: Uuid::nil(),
                version_no: 1,
                title: "first note".into(),
                body_md: Some("close 12,345円 then -6.0%; 73足".into()),
                frontmatter_json: serde_json::Map::new(),
                tags: vec![],
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
            .write_note(
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

    #[backend_test_macros::database_test]
    async fn write_note_rejects_unresolved_price_links_as_invalid_params(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());
        let execution_step_id = Uuid::from_u128(9001);
        let error = server
            .write_note(
                strategy_id,
                Some(execution_step_id),
                WriteNoteParams {
                    note_id: None,
                    title: Some("price reference".into()),
                    body_md: Some("[[price:fictional-code@2030-01-02:close]]".into()),
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect_err("a price link without query_data must be rejected");
        let saved_notes = note::Entity::find().all(&db).await.unwrap();

        assert_eq!(
            (error.code, error.message.as_ref(), saved_notes.len()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "価格参照 [[price:fictional-code@2030-01-02:close]] を実行ステップの query_data から解決できません",
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
        graph.nodes[0].r#ref = Some("stock:demo-code".into());

        let written = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note with refs".into()),
                    body_md: Some(
                        "mentions [[theme:demo-topic]] and [[group:demo-axis/demo-group]]".into(),
                    ),
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
                ("group".to_string(), "demo-axis/demo-group".to_string()),
                ("stock".to_string(), "demo-code".to_string()),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn write_note_update_resyncs_note_refs(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db.clone());

        let created = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("note".into()),
                    body_md: Some("mentions [[stock:demo-code]]".into()),
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
            vec![("stock".to_string(), "demo-code".to_string())],
        );

        server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some(
                        "now mentions [[indicator:demo-indicator]] and [[theme:demo-topic]] and [[sector:demo-industry]]"
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

        assert_eq!(
            note_refs_of(&db, created.note_id).await,
            vec![("indicator".to_string(), "demo-indicator".to_string())],
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
                .write_note(
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
                .write_note(
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
                    warnings: vec![],
                },
                "case {label}",
            );

            let read = server
                .read_note(
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
                    version_id: Uuid::nil(),
                    version_no: 2,
                    title: "original".into(),
                    body_md: Some("v2".into()),
                    frontmatter_json: serde_json::Map::new(),
                    tags: vec![],
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

    #[backend_test_macros::database_test]
    async fn write_note_updates_existing_note(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;

        let result = server
            .write_note(
                strategy_a,
                None,
                WriteNoteParams {
                    note_id: Some(note_id),
                    title: Some("revised note".into()),
                    body_md: Some("revised body".into()),
                    kind: None,
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("update note from another strategy");
        assert_eq!(
            result,
            super::super::dto::WriteNoteResult {
                note_id,
                created: false,
                warnings: vec![],
            },
        );

        let read = server
            .read_note(
                strategy_a,
                ReadNoteParams {
                    note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read updated note from another strategy");
        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id,
                version_id: Uuid::nil(),
                version_no: 2,
                title: "revised note".into(),
                body_md: Some("revised body".into()),
                frontmatter_json: serde_json::Map::new(),
                tags: vec![],
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

    #[backend_test_macros::database_test]
    async fn read_note_returns_existing_note(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let note_id = seed_note(&db, "sample note").await;

        let result = server
            .read_note(
                strategy_a,
                ReadNoteParams {
                    note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note from another strategy");
        assert_eq!(
            normalize_note(result),
            NoteDto {
                note_id,
                version_id: Uuid::nil(),
                version_no: 1,
                title: "sample note".into(),
                body_md: Some("body".into()),
                frontmatter_json: serde_json::Map::new(),
                tags: vec![],
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

    #[backend_test_macros::database_test]
    async fn list_notes_includes_notes_from_all_strategies(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());

        let mut note_ids = Vec::new();
        for (sid, title) in [(strategy_a, "a1"), (strategy_a, "a2"), (strategy_b, "b1")] {
            let written = server
                .write_note(
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
            note_ids.push(written.note_id);
        }
        note_ids.push(crate::testing::insert_test_note(&db, "unscoped", "body").await);
        for (index, note_id) in note_ids.iter().enumerate() {
            set_note_updated_at(
                &db,
                *note_id,
                ts_sentinel() + chrono::Duration::seconds(index as i64),
            )
            .await;
        }

        let result = server
            .list_notes(strategy_a, ListNotesParams::default())
            .await
            .expect("list");
        // 接続先の戦略に関係なく、全件が更新日時の降順で並ぶ
        let titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, vec!["unscoped", "b1", "a2", "a1"]);
    }

    #[backend_test_macros::database_test]
    async fn list_notes_filters_by_kind_and_ref_before_limit(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_note_kind(&db, "sample-kind", false).await;
        insert_note_kind(&db, "other-kind", false).await;
        let server = build_server(db);

        let matching = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("matching".into()),
                    body_md: Some("[[stock:demo-code]]".into()),
                    kind: Some(Some("sample-kind".into())),
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("write matching note");
        for (title, kind, body_md) in [
            ("wrong ref", "sample-kind", "[[stock:other-code]]"),
            ("wrong kind", "other-kind", "[[stock:demo-code]]"),
        ] {
            server
                .write_note(
                    strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: None,
                        title: Some(title.into()),
                        body_md: Some(body_md.into()),
                        kind: Some(Some(kind.into())),
                        frontmatter_json: None,
                        change_reason: None,
                        graphs: None,
                    },
                )
                .await
                .unwrap_or_else(|error| panic!("write {title} note failed: {error}"));
        }

        let result = server
            .list_notes(
                strategy_id,
                ListNotesParams {
                    limit: Some(1),
                    kind: Some("sample-kind".into()),
                    r#ref: Some("stock:demo-code".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("list matching note");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![NoteDto {
                    note_id: matching.note_id,
                    version_id: Uuid::nil(),
                    version_no: 1,
                    title: "matching".into(),
                    body_md: Some("[[stock:demo-code]]".into()),
                    frontmatter_json: serde_json::Map::new(),
                    tags: vec![],
                    kind: Some("sample-kind".into()),
                    status: "unread".into(),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                    graphs: vec![],
                    links: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_filters_by_exact_tag_and_returns_tags(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);
        let matching = server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("tagged note".into()),
                    body_md: Some("body".into()),
                    kind: None,
                    frontmatter_json: Some(
                        serde_json::json!({ "tags": ["demo-focus", "demo-review"] })
                            .as_object()
                            .cloned()
                            .expect("frontmatter is an object"),
                    ),
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("write tagged note");
        server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("similarly named tag note".into()),
                    body_md: Some("body".into()),
                    kind: None,
                    frontmatter_json: Some(
                        serde_json::json!({ "tags": ["demo-focus-extra"] })
                            .as_object()
                            .cloned()
                            .expect("frontmatter is an object"),
                    ),
                    change_reason: None,
                    graphs: None,
                },
            )
            .await
            .expect("write similarly named tag note");

        let result = server
            .list_notes(
                strategy_id,
                ListNotesParams {
                    tag: Some("demo-focus".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("list notes by tag");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![NoteDto {
                    note_id: matching.note_id,
                    version_id: Uuid::nil(),
                    version_no: 1,
                    title: "tagged note".into(),
                    body_md: Some("body".into()),
                    frontmatter_json: serde_json::json!({
                        "tags": ["demo-focus", "demo-review"]
                    })
                    .as_object()
                    .cloned()
                    .expect("frontmatter is an object"),
                    tags: vec!["demo-focus".into(), "demo-review".into()],
                    kind: None,
                    status: "unread".into(),
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    created_at: ts_sentinel(),
                    updated_at: ts_sentinel(),
                    graphs: vec![],
                    links: None,
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_rejects_malformed_refs(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);
        let mut errors = Vec::new();
        for reference in [
            "demo-code",
            "unknown-kind:demo-id",
            "stock:",
            "group:demo-group",
        ] {
            let error = server
                .list_notes(
                    strategy_id,
                    ListNotesParams {
                        r#ref: Some(reference.into()),
                        ..Default::default()
                    },
                )
                .await
                .expect_err("malformed ref should be rejected");
            errors.push((error.code, error.message.to_string()));
        }
        assert_eq!(
            errors,
            vec![
                (
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "ref must use the kind:id format".to_string(),
                ),
                (
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "invalid ref kind: unknown-kind".to_string(),
                ),
                (
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "ref id must not be empty".to_string(),
                ),
                (
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "group ref id must use axis-key/group-key format".to_string(),
                ),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_excludes_pending_notes_by_default(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        let result = server
            .list_notes(fixture.strategy_id, ListNotesParams::default())
            .await
            .expect("list current notes");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![expected_current_note(fixture.current_note_id)],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_filters_pending_notes_by_kind(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        let result = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    kind: Some("sample-kind".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list pending notes by kind");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![expected_latest_pending_note(&fixture, "unread")],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_uses_latest_version_for_pending_notes(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        let result = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list including pending notes");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![
                    expected_latest_pending_note(&fixture, "unread"),
                    expected_current_note(fixture.current_note_id),
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_scans_pending_pages_until_a_ref_matches(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        for index in 0..MAX_LIST_LIMIT {
            server
                .write_note(
                    fixture.strategy_id,
                    None,
                    WriteNoteParams {
                        note_id: None,
                        title: Some(format!("non-matching pending {index}")),
                        body_md: Some("no matching reference".into()),
                        kind: Some(Some("sample-kind".into())),
                        frontmatter_json: None,
                        change_reason: None,
                        graphs: None,
                    },
                )
                .await
                .unwrap_or_else(|error| panic!("write non-matching pending note failed: {error}"));
        }

        let result = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    limit: Some(1),
                    r#ref: Some("stock:demo-code".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list matching pending note after an unmatched page");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![expected_latest_pending_note(&fixture, "unread")],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_filters_pending_notes_by_selected_version_body_ref(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        let selected_ref = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    r#ref: Some("stock:demo-code".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list pending note by selected body reference");
        let previous_version_ref = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    r#ref: Some("group:body-axis/body-group".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list pending note by previous body reference");

        assert_eq!(
            (
                normalize_list_notes(selected_ref).as_json().clone(),
                normalize_list_notes(previous_version_ref).as_json().clone(),
            ),
            (
                serde_json::json!({"notes": [expected_latest_pending_note(&fixture, "unread")]}),
                serde_json::json!({"notes": []}),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_filters_pending_notes_by_graph_ref(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        let result = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    r#ref: Some("group:graph-axis/graph-group".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list pending notes by graph reference");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![expected_latest_pending_note(&fixture, "unread")],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_filters_pending_notes_by_status(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone());
        let fixture = create_pending_note_fixture(&db, &server).await;
        set_latest_note_version_status(&db, fixture.pending_note_id, "rejected").await;
        let result = server
            .list_notes(
                fixture.strategy_id,
                ListNotesParams {
                    status: Some("rejected".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("list rejected pending notes");

        assert_eq!(
            normalize_list_notes(result),
            super::super::dto::ListNotesResult {
                notes: vec![expected_latest_pending_note(&fixture, "rejected")],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn list_notes_skips_pending_versions_with_unparsable_refs(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_note_kind(&db, "sample-kind", true).await;
        let server = build_server(db);
        let mut graph = sample_graph("g1");
        graph.nodes[0].r#ref = Some("unknown-kind:demo-id".into());
        server
            .write_note(
                strategy_id,
                None,
                WriteNoteParams {
                    note_id: None,
                    title: Some("pending with invalid reference".into()),
                    body_md: Some("[[stock:demo-code]]".into()),
                    kind: Some(Some("sample-kind".into())),
                    frontmatter_json: None,
                    change_reason: None,
                    graphs: Some(vec![graph]),
                },
            )
            .await
            .expect("write pending note with invalid reference");
        let result = server
            .list_notes(
                strategy_id,
                ListNotesParams {
                    r#ref: Some("stock:demo-code".into()),
                    include_pending: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("unparsable pending references should not fail the list");

        assert_eq!(result.as_json().clone(), serde_json::json!({"notes": []}));
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
                .write_note(
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
            .list_notes(
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
            .list_notes(
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
            .write_note(
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
            .write_note(
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
            .list_notes(
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
            .write_note(
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
            .list_notes(
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
            .write_note(
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
            .read_note(
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
                version_id: Uuid::nil(),
                version_no: 1,
                title: "note with graph".into(),
                body_md: Some("[[graph:g1]]".into()),
                frontmatter_json: serde_json::Map::new(),
                tags: vec![],
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
            .write_note(
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
            .list_notes(strategy_id, ListNotesParams::default())
            .await
            .expect("list");
        assert_eq!(result.as_json().clone(), serde_json::json!({"notes": []}));
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
            .write_note(
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
            .list_notes(strategy_id, ListNotesParams::default())
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
            .write_note(
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
            .write_note(
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
            .read_note(
                strategy_id,
                ReadNoteParams {
                    note_id: created.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note");

        assert_eq!(
            (err.code, err.message.to_string(), note.body_md.clone()),
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
                .write_note(
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
                .write_note(
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
                .read_note(
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
                    version_id: Uuid::nil(),
                    version_no: 2,
                    title: "t".into(),
                    body_md: Some(expected_body),
                    frontmatter_json: serde_json::Map::new(),
                    tags: vec![],
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
            .write_note(
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

        let current_version = find_current_note_version(&db, created.note_id)
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
            .write_note(
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
            .read_note(
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
                note.body_md.clone(),
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
            .write_note(
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
            .write_note(
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
            .read_note(
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
            .write_note(
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
            .write_note(
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
            .read_note(
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
                version_id: Uuid::nil(),
                version_no: 1,
                title: "t".into(),
                body_md: Some("orig".into()),
                frontmatter_json: serde_json::Map::new(),
                tags: vec![],
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
            .write_note(
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
            .write_note(
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
            .write_note(
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
            .write_note(
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
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(1)),
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
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(1)),
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
                warnings: vec![],
            },
        );

        let read = server
            .read_note(
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
                version_id: Uuid::nil(),
                version_no: 2,
                title: "second".into(),
                body_md: Some("v2".into()),
                frontmatter_json: serde_json::Map::new(),
                tags: vec![],
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
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(1)),
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
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(2)),
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
            .list_notes(strategy_id, ListNotesParams::default())
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
            .write_note(
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
            .write_note(
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
            .list_notes(strategy_id, ListNotesParams::default())
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

        // note B: step ID に紐づく既存ノート
        let note_b = server
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(1)),
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
            .write_note(
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
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(1)),
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
                warnings: vec![],
            },
        );

        let read_a = server
            .read_note(
                strategy_id,
                ReadNoteParams {
                    note_id: note_a.note_id,
                    version_id: None,
                },
            )
            .await
            .expect("read note a");
        let read_b = server
            .read_note(
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
                normalize_note(read_a).body_md.clone(),
                normalize_note(read_b).body_md.clone()
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
            "winner",
            "winner body",
            &Uuid::from_u128(1).to_string(),
        )
        .await;

        let result = server
            .write_note(
                strategy_id,
                Some(Uuid::from_u128(1)),
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
        let current = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (result.as_json().clone(), current.title, current.body_md),
            (
                serde_json::json!({
                    "note_id": note_id,
                    "created": false,
                    "warnings": [],
                }),
                "revised title".into(),
                "revised body".into(),
            ),
        );
    }
}
