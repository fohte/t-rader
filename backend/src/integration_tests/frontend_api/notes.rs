#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use std::sync::Arc;

    use crate::testing::agent_config;
    use crate::testing::find_current_note_version;
    use crate::testing::{
        create_test_server_with_db, create_test_server_with_db_and_agent_client,
        insert_test_strategy,
    };
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use core_application::agent_task_client::{
        AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient,
    };
    use core_application::strategy_task::DEFAULT_PURPOSE;
    use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
    use gateway_postgres::entities::strategy_task;
    use gateway_postgres::entities::{change_history, comment, note, note_version};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    use serde_json::{Value, json};
    use uuid::Uuid;

    const INVALID_NOTE_BODY: &str = "[[bogus:one]] [[bare-demo]]";
    const INVALID_NOTE_TOKEN_ERROR: &str = concat!(
        "ノートのトークンに問題があります:\n",
        "- 本文のトークン \"[[bogus:one]]\": 未知の prefix `bogus` です\n",
        "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
        "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[group:<axis-key>/<group-key>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 3 種のみ使用できます。",
    );

    fn normalize_note_response(mut value: Value) -> Value {
        for key in ["id", "version_id", "created_at", "updated_at"] {
            value[key] = json!(format!("<{key}>"));
        }
        value
    }

    fn normalize_note_version(mut value: Value) -> Value {
        for key in ["id", "note_id", "created_at", "reviewed_at"] {
            if let Some(field) = value.as_object_mut().and_then(|object| object.get_mut(key)) {
                *field = json!(format!("<{key}>"));
            }
        }
        value
    }

    fn expected_note_version_response(version_no: i32, status: &str, is_current: bool) -> Value {
        json!({
            "id": "<id>",
            "note_id": "<note_id>",
            "version_no": version_no,
            "title": format!("version {version_no}"),
            "body_md": "body",
            "frontmatter_json": {},
            "graphs_json": [],
            "status": status,
            "is_current": is_current,
            "change_reason": null,
            "created_by_kind": "llm",
            "execution_id": null,
            "created_at": "<created_at>",
            "reviewed_at": "<reviewed_at>",
        })
    }

    /// strategy_task 行の動的フィールド (id / 時刻 / a2a_task_id) を捨てた比較用ビュー。
    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
    }

    impl TaskShape {
        fn from(row: &strategy_task::Model) -> Self {
            Self {
                strategy_id: row.strategy_id,
                source: row.source.clone(),
                prompt: row.prompt.clone(),
                phase: row.phase.clone(),
            }
        }
    }

    async fn create_test_note_with_body(
        server: &TestServer,
        strategy_id: Uuid,
        title: &str,
        body_md: &str,
    ) -> Uuid {
        create_test_note_with_body_and_creator(server, strategy_id, title, body_md, "human").await
    }

    async fn create_test_note_with_creator(
        server: &TestServer,
        strategy_id: Uuid,
        title: &str,
        created_by_kind: &str,
    ) -> Uuid {
        create_test_note_with_body_and_creator(server, strategy_id, title, "body", created_by_kind)
            .await
    }

    async fn create_test_note_with_body_and_creator(
        server: &TestServer,
        strategy_id: Uuid,
        title: &str,
        body_md: &str,
        created_by_kind: &str,
    ) -> Uuid {
        let res = server
            .post("/api/notes")
            .json(&json!({
                "strategy_id": strategy_id,
                "title": title,
                "body_md": body_md,
                "created_by_kind": created_by_kind,
            }))
            .await;
        let body: Value = res.json();
        let note_id = Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid");
        assert_eq!(
            (res.status_code(), normalize_note_response(body)),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "strategy_id": strategy_id,
                    "title": title,
                    "body_md": body_md,
                    "frontmatter_json": {},
                    "kind": null,
                    "status": if created_by_kind == "human" { "approved" } else { "unread" },
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": created_by_kind,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "graphs_json": [],
                    "execution_id": null,
                }),
            ),
        );
        note_id
    }

    async fn insert_test_version(
        db: &gateway_postgres::DatabaseHandle,
        note_id: Uuid,
        version_no: i32,
        status: &str,
        is_current: bool,
    ) -> Uuid {
        let id = Uuid::new_v4();
        note_version::Entity::insert(note_version::ActiveModel {
            id: Set(id),
            note_id: Set(note_id),
            version_no: Set(version_no),
            title: Set(format!("version {version_no}")),
            body_md: Set("body".into()),
            frontmatter_json: Set(json!({})),
            graphs_json: Set(json!([])),
            status: Set(status.into()),
            is_current: Set(is_current),
            change_reason: Set(None),
            created_by_kind: Set("llm".into()),
            execution_id: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note version");
        id
    }

    async fn create_note_with_approved_initial_version(
        server: &TestServer,
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        title: &str,
    ) -> (Uuid, Uuid) {
        let note_id = create_test_note_with_creator(server, strategy_id, title, "llm").await;
        let initial = find_current_note_version(db, note_id)
            .await
            .unwrap()
            .unwrap();
        note_version::ActiveModel {
            id: Set(initial.id),
            status: Set("approved".into()),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("approve initial version");
        (note_id, initial.id)
    }

    async fn note_version_states(
        db: &gateway_postgres::DatabaseHandle,
        note_id: Uuid,
    ) -> Vec<(i32, String, bool)> {
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .order_by_asc(note_version::Column::VersionNo)
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| (row.version_no, row.status, row.is_current))
            .collect()
    }

    async fn status_change_diff(db: &gateway_postgres::DatabaseHandle, note_id: Uuid) -> Value {
        change_history::Entity::find()
            .filter(change_history::Column::TargetId.eq(note_id))
            .filter(change_history::Column::Op.eq("status_change"))
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .diff_json
    }

    /// strategy を持たないノートは execution (戦略タスク実行) に紐づき得ない、という
    /// note_strategy_id_execution_id_check CHECK 制約の回帰テスト。
    #[backend_test_macros::database_test]
    async fn note_without_strategy_id_rejects_execution_id(db: gateway_postgres::DatabaseHandle) {
        let (db, _server) = create_test_server_with_db(db).await;

        let result = note::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(None),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(Some("exec-1".into())),
        }
        .insert(&db)
        .await;

        assert!(result.is_err());
    }

    #[backend_test_macros::database_test]
    async fn create_note_without_strategy_id_succeeds(db: gateway_postgres::DatabaseHandle) {
        let (_db, server) = create_test_server_with_db(db).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "市況ノート",
                "body_md": "body",
            }))
            .await;
        let mut body: Value = res.json();
        let obj = body.as_object_mut().unwrap();
        for key in ["id", "version_id", "created_at", "updated_at"] {
            obj.insert(key.into(), json!(format!("<{key}>")));
        }
        assert_eq!(
            (res.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "strategy_id": null,
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "title": "市況ノート",
                    "body_md": "body",
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "kind": null,
                    "status": "approved",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_id": null,
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_note_records_history_in_the_same_use_case(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "sample note",
                "body_md": "body",
            }))
            .await;
        let body = res.json::<Value>();
        let note_id = Uuid::parse_str(body["id"].as_str().expect("note id")).expect("uuid");
        let version_id =
            Uuid::parse_str(body["version_id"].as_str().expect("version id")).expect("uuid");
        let normalized_body = normalize_note_response(body);
        let histories = change_history::Entity::find()
            .filter(change_history::Column::TargetId.eq(note_id))
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    row.target_kind,
                    row.target_id,
                    row.actor_kind,
                    row.actor_label,
                    row.op,
                    row.diff_json,
                    row.summary,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            (res.status_code(), normalized_body, histories),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "strategy_id": null,
                    "title": "sample note",
                    "body_md": "body",
                    "frontmatter_json": {},
                    "kind": null,
                    "status": "approved",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "graphs_json": [],
                    "execution_id": null,
                }),
                vec![(
                    "note".to_string(),
                    note_id,
                    "human".to_string(),
                    "user".to_string(),
                    "create".to_string(),
                    json!({
                        "title": "sample note",
                        "strategy_id": null,
                        "from_version_id": null,
                        "to_version_id": version_id,
                        "version_no": 1,
                    }),
                    None,
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn approving_a_version_supersedes_older_pending_versions_and_records_their_ids(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let (note_id, initial_id) =
            create_note_with_approved_initial_version(&server, &db, strategy_id, "review").await;
        let older_pending_id = insert_test_version(&db, note_id, 2, "unread", false).await;
        insert_test_version(&db, note_id, 3, "rejected", false).await;
        let approved_id = insert_test_version(&db, note_id, 4, "unread", false).await;

        let response = server
            .post(&format!("/api/notes/{note_id}/versions/4/approve"))
            .json(&json!({}))
            .await;

        assert_eq!(
            (
                response.status_code(),
                normalize_note_version(response.json()),
                note_version_states(&db, note_id).await,
                status_change_diff(&db, note_id).await,
            ),
            (
                StatusCode::OK,
                expected_note_version_response(4, "approved", true),
                vec![
                    (1, "approved".to_string(), false),
                    (2, "superseded".to_string(), false),
                    (3, "rejected".to_string(), false),
                    (4, "approved".to_string(), true),
                ],
                json!({
                    "from": "unread",
                    "to": "approved",
                    "version_id": approved_id,
                    "previous_current_version_id": initial_id,
                    "superseded_version_ids": [older_pending_id],
                    "label": null,
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn approving_an_older_version_preserves_a_newer_current_version(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = create_test_note_with_creator(&server, strategy_id, "review", "llm").await;
        let initial = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();
        note_version::ActiveModel {
            id: Set(initial.id),
            status: Set("approved".into()),
            is_current: Set(false),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("clear initial current version");
        let superseded_id = insert_test_version(&db, note_id, 2, "unread", false).await;
        let approved_id = insert_test_version(&db, note_id, 3, "unread", false).await;
        let current_id = insert_test_version(&db, note_id, 4, "approved", true).await;
        insert_test_version(&db, note_id, 5, "unread", false).await;

        let response = server
            .post(&format!("/api/notes/{note_id}/versions/3/approve"))
            .json(&json!({}))
            .await;

        assert_eq!(
            (
                response.status_code(),
                normalize_note_version(response.json()),
                note_version_states(&db, note_id).await,
                status_change_diff(&db, note_id).await,
            ),
            (
                StatusCode::OK,
                expected_note_version_response(3, "approved", false),
                vec![
                    (1, "approved".to_string(), false),
                    (2, "superseded".to_string(), false),
                    (3, "approved".to_string(), false),
                    (4, "approved".to_string(), true),
                    (5, "unread".to_string(), false),
                ],
                json!({
                    "from": "unread",
                    "to": "approved",
                    "version_id": approved_id,
                    "previous_current_version_id": current_id,
                    "superseded_version_ids": [superseded_id],
                    "label": null,
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn approving_a_version_does_not_change_another_notes_pending_versions(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let (reviewed_note, _) =
            create_note_with_approved_initial_version(&server, &db, strategy_id, "reviewed").await;
        insert_test_version(&db, reviewed_note, 2, "unread", false).await;
        insert_test_version(&db, reviewed_note, 3, "unread", false).await;
        let (untouched_note, _) =
            create_note_with_approved_initial_version(&server, &db, strategy_id, "untouched").await;
        insert_test_version(&db, untouched_note, 2, "unread", false).await;
        insert_test_version(&db, untouched_note, 3, "unread", false).await;

        let response = server
            .post(&format!("/api/notes/{reviewed_note}/versions/3/approve"))
            .json(&json!({}))
            .await;

        assert_eq!(
            (
                response.status_code(),
                normalize_note_version(response.json()),
                note_version_states(&db, untouched_note).await,
            ),
            (
                StatusCode::OK,
                expected_note_version_response(3, "approved", true),
                vec![
                    (1, "approved".to_string(), true),
                    (2, "unread".to_string(), false),
                    (3, "unread".to_string(), false),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_note_rejects_unknown_kind_as_bad_request(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "sample note",
                "body_md": "body",
                "kind": "sample-kind",
            }))
            .await;
        let response = res.json::<Value>();
        let saved_notes = note::Entity::find().all(&db).await.unwrap();

        assert_eq!(
            (res.status_code(), response, saved_notes.len()),
            (
                StatusCode::BAD_REQUEST,
                json!({"error": "unknown note kind: sample-kind"}),
                0,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_note_rejects_invalid_tokens_without_saving(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "token validation",
                "body_md": INVALID_NOTE_BODY,
            }))
            .await;
        let response = res.json::<Value>();
        let saved_notes = note::Entity::find().all(&db).await.unwrap();

        assert_eq!(
            (res.status_code(), response, saved_notes.len()),
            (
                StatusCode::BAD_REQUEST,
                json!({"error": INVALID_NOTE_TOKEN_ERROR}),
                0,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_note_rejects_invalid_tokens_and_keeps_original_body(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "strategy").await;
        let note_id = create_test_note_with_body(&server, strategy_id, "title", "original").await;

        let res = server
            .patch(&format!("/api/notes/{note_id}"))
            .json(&json!({"body_md": INVALID_NOTE_BODY}))
            .await;
        let response = res.json::<Value>();
        let saved_body = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .map(|version| version.body_md);

        assert_eq!(
            (res.status_code(), response, saved_body),
            (
                StatusCode::BAD_REQUEST,
                json!({"error": INVALID_NOTE_TOKEN_ERROR}),
                Some("original".to_string()),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reject_note_without_strategy_id_does_not_submit_task(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;

        let res = server
            .post("/api/notes")
            .json(&json!({
                "title": "市況ノート",
                "body_md": "body",
                "created_by_kind": "llm",
            }))
            .await;
        let mut created_body = res.json::<Value>();
        let note_id = Uuid::parse_str(created_body["id"].as_str().expect("id")).expect("uuid");
        created_body = normalize_note_response(created_body);
        assert_eq!(
            (res.status_code(), created_body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "strategy_id": null,
                    "title": "市況ノート",
                    "body_md": "body",
                    "frontmatter_json": {},
                    "kind": null,
                    "status": "unread",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "llm",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "graphs_json": [],
                    "execution_id": null,
                }),
            ),
        );
        let version = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();

        let res = server
            .post(&format!(
                "/api/notes/{note_id}/versions/{}/reject",
                version.version_no
            ))
            .json(&json!({"label": "確認事項"}))
            .await;
        assert_eq!(
            (res.status_code(), normalize_note_version(res.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "note_id": "<note_id>",
                    "version_no": version.version_no,
                    "title": "市況ノート",
                    "body_md": "body",
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "status": "rejected",
                    "is_current": true,
                    "change_reason": null,
                    "created_by_kind": "llm",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "reviewed_at": "<reviewed_at>",
                }),
            ),
        );

        let tasks = strategy_task::Entity::find().all(&db).await.unwrap();
        assert_eq!(tasks, vec![]);
    }

    #[backend_test_macros::database_test]
    async fn reject_note_submits_single_review_task_referencing_note(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let note_id = create_test_note_with_creator(&server, strategy_id, "タイトル", "llm").await;
        let version = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();

        let res = server
            .post(&format!(
                "/api/notes/{note_id}/versions/{}/reject",
                version.version_no
            ))
            .json(&json!({"label": "確認事項"}))
            .await;
        assert_eq!(
            (res.status_code(), normalize_note_version(res.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "note_id": "<note_id>",
                    "version_no": version.version_no,
                    "title": "タイトル",
                    "body_md": "body",
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "status": "rejected",
                    "is_current": true,
                    "change_reason": null,
                    "created_by_kind": "llm",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "reviewed_at": "<reviewed_at>",
                }),
            ),
        );

        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
            vec![TaskShape {
                strategy_id,
                source: "review".to_string(),
                prompt: format!(
                    "ノート「タイトル」(id: {note_id}) の v{} (version_id: {}) がレビューで却下されました。理由: 確認事項。付いているコメントを確認し、指摘を反映してください。",
                    version.version_no, version.id,
                ),
                phase: StrategyTaskPhase::Running,
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn rejecting_already_rejected_note_does_not_resubmit(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let note_id = create_test_note_with_creator(&server, strategy_id, "t", "llm").await;
        let version = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();

        let first = server
            .post(&format!(
                "/api/notes/{note_id}/versions/{}/reject",
                version.version_no
            ))
            .json(&json!({"label": "確認事項"}))
            .await;
        assert_eq!(
            (first.status_code(), normalize_note_version(first.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "note_id": "<note_id>",
                    "version_no": version.version_no,
                    "title": "t",
                    "body_md": "body",
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "status": "rejected",
                    "is_current": true,
                    "change_reason": null,
                    "created_by_kind": "llm",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "reviewed_at": "<reviewed_at>",
                }),
            ),
        );
        let second = server
            .post(&format!(
                "/api/notes/{note_id}/versions/{}/reject",
                version.version_no
            ))
            .json(&json!({"label": "確認事項"}))
            .await;
        assert_response_eq(
            &second,
            StatusCode::CONFLICT,
            Some(json!({
                "error": format!(
                    "note version {note_id}/{} is not pending",
                    version.version_no
                )
            })),
        );

        let tasks = strategy_task::Entity::find()
            .filter(strategy_task::Column::StrategyId.eq(strategy_id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[backend_test_macros::database_test]
    async fn reject_note_leaves_status_unchanged_when_agent_submission_fails(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let note_id = create_test_note_with_creator(&server, strategy_id, "t", "llm").await;
        let version = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();

        let res = server
            .post(&format!(
                "/api/notes/{note_id}/versions/{}/reject",
                version.version_no
            ))
            .json(&json!({"label": "確認事項"}))
            .await;
        assert_response_eq(
            &res,
            StatusCode::SERVICE_UNAVAILABLE,
            Some(json!({ "error": "agent task client is not configured" })),
        );

        let current_version = find_current_note_version(&db, note_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current_version.status, "unread");
    }

    #[backend_test_macros::database_test]
    async fn update_note_keeps_comment_anchored_to_original_version(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = create_test_note_with_body(
            &server,
            strategy_id,
            "note",
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;
        let version_id = find_current_note_version(&db, note_id)
            .await
            .expect("find current version")
            .expect("current version exists")
            .id;

        let created_comment = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": version_id,
                "body": "fix this",
                "anchor_text": "line two",
                "anchor_side": "new",
                "start_line": 2,
                "end_line": 2,
            }))
            .await;
        let mut created_comment_body = created_comment.json::<Value>();
        let comment_id =
            Uuid::parse_str(created_comment_body["id"].as_str().expect("id")).expect("uuid");
        created_comment_body["id"] = json!("<id>");
        created_comment_body["created_at"] = json!("<created_at>");
        assert_eq!(
            (created_comment.status_code(), created_comment_body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_kind": "note_version",
                    "target_id": version_id,
                    "parent_id": null,
                    "body": "fix this",
                    "author_kind": "human",
                    "author_label": "user",
                    "created_at": "<created_at>",
                    "resolved": false,
                    "anchor_text": "line two",
                    "start_line": 2,
                    "end_line": 2,
                    "anchor_side": "new",
                }),
            ),
        );

        let res = server
            .patch(&format!("/api/notes/{note_id}"))
            .json(&json!({
                "body_md": indoc::indoc! {"
                    prefix
                    line one
                    line two
                    line three"},
            }))
            .await;
        assert_eq!(
            (res.status_code(), normalize_note_response(res.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 2,
                    "is_current": true,
                    "strategy_id": strategy_id,
                    "title": "note",
                    "body_md": indoc::indoc! {"
                        prefix
                        line one
                        line two
                        line three"},
                    "frontmatter_json": {},
                    "kind": null,
                    "status": "approved",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "graphs_json": [],
                    "execution_id": null,
                }),
            ),
        );

        let mut updated_comment = comment::Entity::find_by_id(comment_id)
            .one(&db)
            .await
            .expect("query")
            .expect("comment exists");
        updated_comment.created_at = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset();
        assert_eq!(
            updated_comment,
            comment::Model {
                id: comment_id,
                target_kind: "note_version".into(),
                target_id: version_id,
                parent_id: None,
                body: "fix this".into(),
                author_kind: "human".into(),
                author_label: "user".into(),
                created_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
                resolved: false,
                anchor_text: Some("line two".into()),
                anchor_side: Some("new".into()),
                start_line: Some(2),
                end_line: Some(2),
            },
        );
    }
}
