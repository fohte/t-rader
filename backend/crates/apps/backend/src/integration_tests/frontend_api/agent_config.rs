#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_agent_config};
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use gateway_postgres::entities::{agent_config, change_history};
    use rstest::{fixture, rstest};
    use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, SqlxPostgresConnector};
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use uuid::Uuid;

    struct AgentConfigHistoryTestContext {
        server: axum_test::TestServer,
        db: gateway_postgres::DatabaseHandle,
        _database_test_lock: Option<gateway_postgres::DatabaseHandle>,
        purpose: String,
        id: String,
        created_status: StatusCode,
    }

    async fn create_agent_config_history_test_context(
        db: gateway_postgres::DatabaseHandle,
        database_test_lock: Option<gateway_postgres::DatabaseHandle>,
        purpose: String,
    ) -> AgentConfigHistoryTestContext {
        let server = create_test_server(db.clone()).await;
        let created = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": purpose.clone() }))
            .await;
        let created_status = created.status_code();
        let created_body = created.json::<Value>();
        let id = created_body["id"]
            .as_str()
            .expect("created config id")
            .to_string();
        AgentConfigHistoryTestContext {
            server,
            db,
            _database_test_lock: database_test_lock,
            purpose,
            id,
            created_status,
        }
    }

    #[fixture]
    async fn agent_config_history_context() -> AgentConfigHistoryTestContext {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        create_agent_config_history_test_context(db, None, "example-purpose".to_string()).await
    }

    #[fixture]
    async fn concurrent_agent_config_history_context() -> AgentConfigHistoryTestContext {
        let database_test_lock = gateway_postgres::test_support::create_test_transaction().await;
        let pool = gateway_postgres::test_support::create_test_pool_with_max_connections(4).await;
        let db = gateway_postgres::DatabaseHandle::from(
            SqlxPostgresConnector::from_sqlx_postgres_pool(pool),
        );
        create_agent_config_history_test_context(
            db,
            Some(database_test_lock),
            format!("test-{}", Uuid::new_v4().simple()),
        )
        .await
    }

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        if let Some(items) = value.as_array_mut() {
            for item in items {
                *item = normalize(std::mem::take(item));
            }
        }
        value
    }

    fn snapshot(
        id: &str,
        purpose: &str,
        agents_md: &str,
        skills: Value,
        agent_graph: &str,
    ) -> Value {
        json!({
            "id": id,
            "purpose": purpose,
            "agents_md": agents_md,
            "skills": skills,
            "agent_graph": agent_graph,
        })
    }

    fn expected_history_row(
        target_id: &str,
        op: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Value {
        json!({
            "id": "<history-id>",
            "target_kind": "agent_config",
            "target_id": target_id,
            "actor_kind": "human",
            "actor_label": "user",
            "op": op,
            "diff_json": {
                "format": "definition_snapshot",
                "version": 1,
                "before": before,
                "after": after,
            },
            "summary": null,
            "created_at": "<history-created-at>",
        })
    }

    fn normalize_history_rows(rows: &mut [Value]) {
        for row in rows {
            row["id"] = json!("<history-id>");
            row["created_at"] = json!("<history-created-at>");
        }
    }

    #[backend_test_macros::database_test]
    async fn create_and_list_roundtrip(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await;
        assert_eq!(
            (created.status_code(), normalize(created.json())),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "purpose": "explore",
                    "agents_md": "",
                    "skills": {},
                    "agent_graph": "",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );

        let list = server.get("/api/agent-configs").await;
        assert_eq!(
            (list.status_code(), normalize(list.json())),
            (
                StatusCode::OK,
                json!([{
                    "id": "<id>",
                    "purpose": "explore",
                    "agents_md": "",
                    "skills": {},
                    "agent_graph": "",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }]),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn create_updates_and_delete_return_full_definition_snapshots(
        #[future] agent_config_history_context: AgentConfigHistoryTestContext,
    ) {
        let context = agent_config_history_context.await;
        let server = &context.server;
        let id = context.id.clone();
        let initial = snapshot(&id, "example-purpose", "", json!({}), "");

        let agents_md = indoc::indoc! {"
            First fabricated line.

            Final fabricated line."};
        let agents_md_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({ "content": agents_md }))
            .await;
        let with_agents_md = snapshot(&id, "example-purpose", agents_md, json!({}), "");

        let skills = json!({
            "sample_skill": "fabricated skill content",
            "second_skill": "second fabricated content",
        });
        let skills_put = server
            .put("/api/agent-configs/example-purpose/skills")
            .json(&json!({ "skills": skills }))
            .await;
        let with_skills = snapshot(&id, "example-purpose", agents_md, skills.clone(), "");

        let agent_graph = indoc::indoc! {"
            timeout_minutes: 21
            phases:
              - key: sample-phase
                label: Sample phase
                model: fictional-model-phase
                prompt: fabricated example text"};
        let graph_put = server
            .put("/api/agent-configs/example-purpose/agent-graph")
            .json(&json!({ "content": agent_graph }))
            .await;
        let with_graph = snapshot(&id, "example-purpose", agents_md, skills, agent_graph);

        let deleted = server.delete("/api/agent-configs/example-purpose").await;
        let history = server
            .get(&format!(
                "/api/history?target_kind=agent_config&target_id={id}"
            ))
            .await;
        let mut rows = history.json::<Vec<Value>>();
        normalize_history_rows(&mut rows);
        rows.sort_by_key(Value::to_string);
        let mut expected_rows = vec![
            expected_history_row(&id, "create", None, Some(initial.clone())),
            expected_history_row(&id, "update", Some(initial), Some(with_agents_md.clone())),
            expected_history_row(
                &id,
                "update",
                Some(with_agents_md),
                Some(with_skills.clone()),
            ),
            expected_history_row(&id, "update", Some(with_skills), Some(with_graph.clone())),
            expected_history_row(&id, "delete", Some(with_graph), None),
        ];
        expected_rows.sort_by_key(Value::to_string);

        assert_eq!(
            (
                context.created_status,
                agents_md_put.status_code(),
                skills_put.status_code(),
                graph_put.status_code(),
                deleted.status_code(),
                history.status_code(),
                rows,
            ),
            (
                StatusCode::CREATED,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::NO_CONTENT,
                StatusCode::OK,
                expected_rows,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn history_snapshot_can_restore_definition_through_existing_puts(
        #[future] agent_config_history_context: AgentConfigHistoryTestContext,
    ) {
        let context = agent_config_history_context.await;
        let server = &context.server;
        let id = context.id.clone();

        let original_agents_md = indoc::indoc! {"
            Original fabricated guidance.

            Keep these lines."};
        let original_skills = json!({
            "first_skill": "first fabricated content",
            "second_skill": "second fabricated content",
        });
        let original_agent_graph = indoc::indoc! {"
            timeout_minutes: 34
            phases:
              - key: original-phase
                label: Original phase
                model: fictional-model-original
                prompt: fabricated original text"};
        let baseline = snapshot(
            &id,
            "example-purpose",
            original_agents_md,
            original_skills.clone(),
            original_agent_graph,
        );

        let baseline_agents_md_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({ "content": original_agents_md }))
            .await;
        let baseline_skills_put = server
            .put("/api/agent-configs/example-purpose/skills")
            .json(&json!({ "skills": original_skills }))
            .await;
        let baseline_graph_put = server
            .put("/api/agent-configs/example-purpose/agent-graph")
            .json(&json!({ "content": original_agent_graph }))
            .await;

        let added_skill_put = server
            .put("/api/agent-configs/example-purpose/skills/added_skill")
            .json(&json!({ "content": "added fabricated content" }))
            .await;
        let removed_skill = server
            .delete("/api/agent-configs/example-purpose/skills/first_skill")
            .await;
        let replaced_skills_put = server
            .put("/api/agent-configs/example-purpose/skills")
            .json(&json!({
                "skills": { "replacement_skill": "replacement fabricated content" }
            }))
            .await;
        let empty_agents_md_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({ "content": "" }))
            .await;
        let empty_definition = snapshot(
            &id,
            "example-purpose",
            "",
            json!({ "replacement_skill": "replacement fabricated content" }),
            original_agent_graph,
        );
        let temporary_agents_md = "temporary fabricated content";
        let nonempty_after_empty_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({ "content": temporary_agents_md }))
            .await;
        let changed_agent_graph = indoc::indoc! {"
            timeout_minutes: 55
            phases:
              - key: changed-phase
                label: Changed phase
                model: fictional-model-changed
                prompt: fabricated changed text"};
        let changed_graph_put = server
            .put("/api/agent-configs/example-purpose/agent-graph")
            .json(&json!({ "content": changed_agent_graph }))
            .await;

        let history = server
            .get(&format!(
                "/api/history?target_kind=agent_config&target_id={id}"
            ))
            .await;
        let history_rows = history.json::<Vec<Value>>();
        let first_change = history_rows
            .iter()
            .find(|row| row["diff_json"]["before"] == baseline)
            .cloned()
            .unwrap_or(Value::Null);
        let empty_change = history_rows
            .iter()
            .find(|row| row["diff_json"]["before"] == empty_definition)
            .cloned()
            .unwrap_or(Value::Null);
        let first_change_after = snapshot(
            &id,
            "example-purpose",
            original_agents_md,
            json!({
                "added_skill": "added fabricated content",
                "first_skill": "first fabricated content",
                "second_skill": "second fabricated content",
            }),
            original_agent_graph,
        );
        let first_change = normalize_history_row(first_change);
        let empty_change = normalize_history_row(empty_change);
        let restored_empty_before = empty_change["diff_json"]["before"].clone();

        let restore_empty_skills_put = server
            .put("/api/agent-configs/example-purpose/skills")
            .json(&json!({ "skills": restored_empty_before["skills"].clone() }))
            .await;
        let restore_empty_agents_md_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({
                "content": restored_empty_before["agents_md"].as_str().unwrap_or_default()
            }))
            .await;
        let restore_empty_graph_put = server
            .put("/api/agent-configs/example-purpose/agent-graph")
            .json(&json!({
                "content": restored_empty_before["agent_graph"].as_str().unwrap_or_default()
            }))
            .await;
        let restored_empty = server.get("/api/agent-configs/example-purpose").await;
        let restored_empty_body = normalize(restored_empty.json::<Value>());

        let restored_before = first_change["diff_json"]["before"].clone();

        let restore_skills_put = server
            .put("/api/agent-configs/example-purpose/skills")
            .json(&json!({ "skills": restored_before["skills"].clone() }))
            .await;
        let restore_agents_md_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({
                "content": restored_before["agents_md"].as_str().unwrap_or_default()
            }))
            .await;
        let restore_graph_put = server
            .put("/api/agent-configs/example-purpose/agent-graph")
            .json(&json!({
                "content": restored_before["agent_graph"].as_str().unwrap_or_default()
            }))
            .await;
        let restored = server.get("/api/agent-configs/example-purpose").await;
        let restored_body = normalize(restored.json::<Value>());

        assert_eq!(
            (
                [
                    context.created_status,
                    baseline_agents_md_put.status_code(),
                    baseline_skills_put.status_code(),
                    baseline_graph_put.status_code(),
                    added_skill_put.status_code(),
                    removed_skill.status_code(),
                    replaced_skills_put.status_code(),
                    empty_agents_md_put.status_code(),
                    nonempty_after_empty_put.status_code(),
                    changed_graph_put.status_code(),
                ],
                history.status_code(),
                history_rows.len(),
                first_change,
                empty_change,
                [
                    restore_empty_skills_put.status_code(),
                    restore_empty_agents_md_put.status_code(),
                    restore_empty_graph_put.status_code(),
                ],
                restored_empty.status_code(),
                restored_empty_body,
                [
                    restore_skills_put.status_code(),
                    restore_agents_md_put.status_code(),
                    restore_graph_put.status_code(),
                ],
                restored.status_code(),
                restored_body,
            ),
            (
                [
                    StatusCode::CREATED,
                    StatusCode::OK,
                    StatusCode::OK,
                    StatusCode::OK,
                    StatusCode::OK,
                    StatusCode::NO_CONTENT,
                    StatusCode::OK,
                    StatusCode::OK,
                    StatusCode::OK,
                    StatusCode::OK,
                ],
                StatusCode::OK,
                10,
                expected_history_row(&id, "update", Some(baseline), Some(first_change_after),),
                expected_history_row(
                    &id,
                    "update",
                    Some(empty_definition),
                    Some(snapshot(
                        &id,
                        "example-purpose",
                        temporary_agents_md,
                        json!({ "replacement_skill": "replacement fabricated content" }),
                        original_agent_graph,
                    )),
                ),
                [StatusCode::OK, StatusCode::OK, StatusCode::OK],
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "purpose": "example-purpose",
                    "agents_md": "",
                    "skills": { "replacement_skill": "replacement fabricated content" },
                    "agent_graph": original_agent_graph,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
                [StatusCode::OK, StatusCode::OK, StatusCode::OK],
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "purpose": "example-purpose",
                    "agents_md": original_agents_md,
                    "skills": {
                        "first_skill": "first fabricated content",
                        "second_skill": "second fabricated content",
                    },
                    "agent_graph": original_agent_graph,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    fn normalize_history_row(mut row: Value) -> Value {
        row["id"] = json!("<history-id>");
        row["created_at"] = json!("<history-created-at>");
        row
    }

    #[rstest]
    #[tokio::test]
    async fn failed_history_insert_rolls_back_agent_config_update(
        #[future] agent_config_history_context: AgentConfigHistoryTestContext,
    ) {
        let context = agent_config_history_context.await;
        let server = &context.server;
        let db = &context.db;
        let id = context.id.clone();

        db.execute_unprepared(
            "CREATE FUNCTION reject_agent_config_history_for_test() RETURNS trigger \
             LANGUAGE plpgsql AS $$ BEGIN \
             IF NEW.target_kind = 'agent_config' THEN \
             RAISE EXCEPTION 'test history insert failure'; \
             END IF; RETURN NEW; END; $$",
        )
        .await
        .expect("create test history failure function");
        db.execute_unprepared(
            "CREATE TRIGGER reject_agent_config_history_for_test \
             BEFORE INSERT ON change_history FOR EACH ROW \
             EXECUTE FUNCTION reject_agent_config_history_for_test()",
        )
        .await
        .expect("create test history failure trigger");

        let failed_put = server
            .put("/api/agent-configs/example-purpose/agents-md")
            .json(&json!({ "content": "uncommitted fabricated content" }))
            .await;

        db.execute_unprepared(
            "DROP TRIGGER reject_agent_config_history_for_test ON change_history",
        )
        .await
        .expect("drop test history failure trigger");
        db.execute_unprepared("DROP FUNCTION reject_agent_config_history_for_test()")
            .await
            .expect("drop test history failure function");

        let current = server.get("/api/agent-configs/example-purpose").await;
        let history = server
            .get(&format!(
                "/api/history?target_kind=agent_config&target_id={id}"
            ))
            .await;
        let mut rows = history.json::<Vec<Value>>();
        normalize_history_rows(&mut rows);

        assert_eq!(
            (
                context.created_status,
                failed_put.status_code(),
                current.status_code(),
                normalize(current.json::<Value>()),
                history.status_code(),
                rows,
            ),
            (
                StatusCode::CREATED,
                StatusCode::INTERNAL_SERVER_ERROR,
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "purpose": "example-purpose",
                    "agents_md": "",
                    "skills": {},
                    "agent_graph": "",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
                StatusCode::OK,
                vec![expected_history_row(
                    &id,
                    "create",
                    None,
                    Some(snapshot(&id, "example-purpose", "", json!({}), "")),
                )],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn concurrent_updates_record_the_actual_previous_definition(
        #[future] concurrent_agent_config_history_context: AgentConfigHistoryTestContext,
    ) {
        let context = concurrent_agent_config_history_context.await;
        let AgentConfigHistoryTestContext {
            server,
            db,
            _database_test_lock,
            purpose,
            id,
            created_status,
        } = context;
        let server = Arc::new(server);
        let config_path = format!("/api/agent-configs/{purpose}");
        let agents_md_path = format!("{config_path}/agents-md");
        let initial_content = "starting fabricated content";
        let initial_put = server
            .put(&agents_md_path)
            .json(&json!({ "content": initial_content }))
            .await;

        let first_content = "competing fabricated content alpha";
        let second_content = "competing fabricated content beta";
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = barrier.clone();
        let second_barrier = barrier.clone();
        let first_server = server.clone();
        let second_server = server.clone();
        let first_agents_md_path = agents_md_path.clone();
        let second_agents_md_path = agents_md_path.clone();
        let (first_update, second_update) = tokio::join!(
            async move {
                first_barrier.wait().await;
                first_server
                    .put(&first_agents_md_path)
                    .json(&json!({ "content": first_content }))
                    .await
            },
            async move {
                second_barrier.wait().await;
                second_server
                    .put(&second_agents_md_path)
                    .json(&json!({ "content": second_content }))
                    .await
            }
        );

        let skills_path = format!("{config_path}/skills");
        let first_skill_content = "first concurrent fabricated skill";
        let second_skill_content = "second concurrent fabricated skill";
        let skill_barrier = Arc::new(Barrier::new(2));
        let first_skill_barrier = skill_barrier.clone();
        let second_skill_barrier = skill_barrier.clone();
        let first_skill_server = server.clone();
        let second_skill_server = server.clone();
        let first_skill_path = format!("{skills_path}/first_skill");
        let second_skill_path = format!("{skills_path}/second_skill");
        let (first_skill_update, second_skill_update) = tokio::join!(
            async move {
                first_skill_barrier.wait().await;
                first_skill_server
                    .put(&first_skill_path)
                    .json(&json!({ "content": first_skill_content }))
                    .await
            },
            async move {
                second_skill_barrier.wait().await;
                second_skill_server
                    .put(&second_skill_path)
                    .json(&json!({ "content": second_skill_content }))
                    .await
            }
        );

        let current = server.get(&config_path).await;
        let current_body = current.json::<Value>();
        let final_content = current_body["agents_md"]
            .as_str()
            .expect("current agents_md")
            .to_string();
        let history = server
            .get(&format!(
                "/api/history?target_kind=agent_config&target_id={id}"
            ))
            .await;
        let history_rows = history.json::<Vec<Value>>();
        let transitions = history_rows
            .iter()
            .filter_map(|row| {
                let before = row["diff_json"]["before"]["agents_md"].as_str()?;
                let after = row["diff_json"]["after"]["agents_md"].as_str()?;
                (before != after && [first_content, second_content].contains(&after))
                    .then(|| (before.to_string(), after.to_string()))
            })
            .collect::<Vec<_>>();
        let mut transitions = transitions;
        transitions.sort();
        let (expected_final, mut expected_transitions) = match final_content.as_str() {
            "competing fabricated content alpha" => (
                first_content,
                vec![
                    (initial_content.to_string(), second_content.to_string()),
                    (second_content.to_string(), first_content.to_string()),
                ],
            ),
            "competing fabricated content beta" => (
                second_content,
                vec![
                    (initial_content.to_string(), first_content.to_string()),
                    (first_content.to_string(), second_content.to_string()),
                ],
            ),
            _ => ("unexpected fabricated content", Vec::new()),
        };
        expected_transitions.sort();

        let final_skills = json!({
            "first_skill": first_skill_content,
            "second_skill": second_skill_content,
        });
        let mut skill_transitions = history_rows
            .iter()
            .filter_map(|row| {
                let before = row["diff_json"]["before"]["skills"].as_object()?;
                let after = row["diff_json"]["after"]["skills"].as_object()?;
                after
                    .values()
                    .any(|value| {
                        value.as_str().is_some_and(|content| {
                            [first_skill_content, second_skill_content].contains(&content)
                        })
                    })
                    .then(|| (Value::Object(before.clone()), Value::Object(after.clone())))
            })
            .collect::<Vec<_>>();
        skill_transitions.sort_by_key(|(before, after)| (before.to_string(), after.to_string()));
        let intermediate_skills = skill_transitions
            .iter()
            .map(|(_, after)| after)
            .find(|after| after.as_object().is_some_and(|skills| skills.len() == 1))
            .cloned()
            .unwrap_or(Value::Null);
        let mut expected_skill_transitions = vec![
            (json!({}), intermediate_skills.clone()),
            (intermediate_skills, final_skills.clone()),
        ];
        expected_skill_transitions
            .sort_by_key(|(before, after)| (before.to_string(), after.to_string()));
        let current_body = normalize(current_body);

        let target_id = Uuid::parse_str(&id).expect("created config id is a UUID");
        change_history::Entity::delete_many()
            .filter(change_history::Column::TargetKind.eq("agent_config"))
            .filter(change_history::Column::TargetId.eq(target_id))
            .exec(&db)
            .await
            .expect("delete concurrent test history");
        agent_config::Entity::delete_many()
            .filter(agent_config::Column::Id.eq(target_id))
            .exec(&db)
            .await
            .expect("delete concurrent test config");

        assert_eq!(
            (
                created_status,
                initial_put.status_code(),
                first_update.status_code(),
                second_update.status_code(),
                first_skill_update.status_code(),
                second_skill_update.status_code(),
                current.status_code(),
                history.status_code(),
                current_body,
                final_content,
                transitions,
                skill_transitions,
            ),
            (
                StatusCode::CREATED,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "purpose": purpose,
                    "agents_md": expected_final,
                    "skills": final_skills,
                    "agent_graph": "",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
                expected_final.to_string(),
                expected_transitions,
                expected_skill_transitions,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_purpose_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = json!({ "purpose": "explore" });
        create_agent_config(&server, "explore").await;
        let res = server.post("/api/agent-configs").json(&body).await;
        assert_response_eq(
            &res,
            StatusCode::CONFLICT,
            Some(json!({ "error": "agent_config with purpose 'explore' already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn invalid_purpose_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "Bad Purpose" }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(
                json!({ "error": "purpose must match ^[a-z0-9][a-z0-9_-]*$ (got 'Bad Purpose')" }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_agent_config_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing").await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'missing' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_agent_config_removes_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let deleted = server.delete("/api/agent-configs/explore").await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);
        let missing = server.get("/api/agent-configs/explore").await;
        assert_response_eq(
            &missing,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'explore' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_agents_md_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let body = "# 方針\n慎重に運用する";
        let put = server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": body }))
            .await;
        assert_response_eq(&put, StatusCode::OK, Some(json!({ "content": body })));

        let get = server.get("/api/agent-configs/explore/agents-md").await;
        assert_response_eq(&get, StatusCode::OK, Some(json!({ "content": body })));
    }

    #[backend_test_macros::database_test]
    async fn agents_md_get_404_for_unknown_purpose(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing/agents-md").await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'missing' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn single_skill_add_update_delete_lifecycle(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let first = server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "first" }))
            .await;
        assert_response_eq(&first, StatusCode::OK, Some(json!({ "content": "first" })));
        let second = server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "second" }))
            .await;
        assert_response_eq(
            &second,
            StatusCode::OK,
            Some(json!({ "content": "second" })),
        );
        let review = server
            .put("/api/agent-configs/explore/skills/review")
            .json(&json!({ "content": "rev" }))
            .await;
        assert_response_eq(&review, StatusCode::OK, Some(json!({ "content": "rev" })));

        let after_adds = server.get("/api/agent-configs/explore/skills").await;
        assert_response_eq(
            &after_adds,
            StatusCode::OK,
            Some(json!({ "skills": { "scout": "second", "review": "rev" } })),
        );

        let deleted = server
            .delete("/api/agent-configs/explore/skills/scout")
            .await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);

        let after_del = server.get("/api/agent-configs/explore/skills").await;
        assert_response_eq(
            &after_del,
            StatusCode::OK,
            Some(json!({ "skills": { "review": "rev" } })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_unknown_skill_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;
        let res = server
            .delete("/api/agent-configs/explore/skills/missing")
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "skill 'missing' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_agent_graph_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let yaml = indoc::indoc! {"
            timeout_minutes: 360
            tool_limits:
              search_web: 4
            phases:
              - key: plan
                label: 調査計画
                model: sample-model-plan
                timeout_minutes: 1
                prompt: 仮説を立てよ
        "};
        let put = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": yaml }))
            .await;
        assert_response_eq(&put, StatusCode::OK, Some(json!({ "content": yaml })));

        let get = server.get("/api/agent-configs/explore/agent-graph").await;
        assert_response_eq(&get, StatusCode::OK, Some(json!({ "content": yaml })));
    }

    #[backend_test_macros::database_test]
    async fn put_agent_graph_rejects_invalid_yaml(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let res = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": "phases: [" }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({
                "error": "agent_graph is not valid YAML: did not find expected node content at line 2 column 1, while parsing a flow node"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_agent_graph_rejects_timeout_outside_range_without_saving(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;
        let yaml = indoc::indoc! {"
            timeout_minutes: 361
            phases:
              - key: plan
                label: Plan
                model: example-model-plan
                prompt: Make a plan
        "};

        let put = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": yaml }))
            .await;
        let get = server.get("/api/agent-configs/explore/agent-graph").await;

        assert_eq!(
            (put.status_code(), put.json(), get.json()),
            (
                StatusCode::BAD_REQUEST,
                json!({ "error": "agent_graph: timeout_minutes must be between 1 and 360, got 361" }),
                json!({ "content": "" }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_agent_graph_rejects_negative_tool_limit_without_saving(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;
        let yaml = indoc::indoc! {"
            tool_limits:
              search_web: -1
            phases:
              - key: plan
                label: Plan
                model: example-model-plan
                prompt: ''
        "};

        let put = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": yaml }))
            .await;
        let get = server.get("/api/agent-configs/explore/agent-graph").await;

        assert_eq!(
            (put.status_code(), put.json(), get.json()),
            (
                StatusCode::BAD_REQUEST,
                json!({ "error": "agent_graph is not valid YAML: tool_limits.search_web: invalid type: integer `-1`, expected u32 at line 2 column 15" }),
                json!({ "content": "" }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_agent_config_bundle_returns_agents_md_skills_and_agent_graph(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let agents_md = indoc::indoc! {"
            # 方針
            慎重に運用する"};
        let put_agents_md = server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": agents_md }))
            .await;
        assert_response_eq(
            &put_agents_md,
            StatusCode::OK,
            Some(json!({ "content": agents_md })),
        );
        let put_skill = server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "scout body" }))
            .await;
        assert_response_eq(
            &put_skill,
            StatusCode::OK,
            Some(json!({ "content": "scout body" })),
        );

        let res = server.get("/api/agent-configs/explore/agent-config").await;
        assert_response_eq(
            &res,
            StatusCode::OK,
            Some(json!({
                "agents_md": agents_md,
                "skills": { "scout": "scout body" },
                "agent_graph": "",
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_agent_config_bundle_404_for_unknown_purpose(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing/agent-config").await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'missing' not found" })),
        );
    }
}
