use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use core_application::markdown_lint::MarkdownLintService;
use core_application::note::NoteReadUseCases;
use core_application::persistence::PersistenceError;
use core_application::refs::{
    IndicatorRef, RefRepository, RefRepositoryError, RefSearchMatch, RefTerm, RefUseCases,
    SharedRefRepository, StockRef,
};
use core_application::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};
use gateway_postgres::{DatabaseHandle, PostgresNoteReadQuery, PostgresUnitOfWork};
use serde_json::{Value, json};
use uuid::Uuid;

use super::normalize_note_response;
use crate::testing::{
    create_test_server_with_db, create_test_server_with_note_reads, insert_test_stock,
};

const RAW_BODY: &str = "[[stock:DEMO-A]] 架空工業: 最初の段落\n続く行";
const DISPLAY_BODY: &str = "[[stock:DEMO-A]]: 最初の段落\n続く行";

async fn create_note(server: &axum_test::TestServer, body_md: &str) -> (Uuid, Uuid) {
    let response = server
        .post("/api/notes")
        .json(&json!({
            "title": "架空の分析ノート",
            "body_md": body_md,
        }))
        .await;
    let actual = response.json::<Value>();
    let note_id =
        Uuid::parse_str(actual["id"].as_str().expect("note id")).expect("note id is a UUID");
    let version_id = Uuid::parse_str(actual["version_id"].as_str().expect("version id"))
        .expect("version id is a UUID");

    assert_eq!(
        (response.status_code(), normalize_note_response(actual)),
        (
            StatusCode::CREATED,
            json!({
                "id": "<id>",
                "version_id": "<version_id>",
                "version_no": 1,
                "is_current": true,
                "title": "架空の分析ノート",
                "body_md": body_md,
                "frontmatter_json": {},
                "resolved_price_references_json": {},
                "tags": [],
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
    (note_id, version_id)
}

async fn get_versions(server: &axum_test::TestServer, note_id: Uuid) -> Value {
    server
        .get(&format!("/api/notes/{note_id}/versions"))
        .await
        .json()
}

async fn get_comments(server: &axum_test::TestServer, version_id: Uuid) -> Value {
    server
        .get(&format!(
            "/api/comments?target_kind=note_version&target_id={version_id}"
        ))
        .await
        .json()
}

fn expected_detail(
    version_id: &str,
    version_no: i64,
    is_current: bool,
    title: &str,
    body_md: &str,
    display_body_md: &str,
) -> Value {
    json!({
        "id": "<id>",
        "version_id": version_id,
        "version_no": version_no,
        "is_current": is_current,
        "title": title,
        "body_md": body_md,
        "display_body_md": display_body_md,
        "frontmatter_json": {},
        "resolved_price_references_json": {},
        "tags": [],
        "kind": null,
        "status": "approved",
        "trigger": null,
        "trigger_label": null,
        "created_by_kind": "human",
        "created_at": "<created_at>",
        "updated_at": "<updated_at>",
        "graphs_json": [],
        "execution_id": null,
    })
}

fn normalize_comment(mut value: Value) -> Value {
    for key in ["id", "created_at"] {
        value[key] = json!(format!("<{key}>"));
    }
    value
}

fn normalize_note_versions(mut value: Value) -> Value {
    for version in value.as_array_mut().expect("versions are an array") {
        for key in ["id", "created_at"] {
            version[key] = json!(format!("<{key}>"));
        }
    }
    value
}

fn normalize_comments(mut value: Value) -> Value {
    for comment in value.as_array_mut().expect("comments are an array") {
        for key in ["id", "created_at"] {
            comment[key] = json!(format!("<{key}>"));
        }
    }
    value
}

#[backend_test_macros::database_test]
async fn get_note_displays_a_corrected_body_without_changing_versions_or_comments(
    db: DatabaseHandle,
) {
    let (db, server) = create_test_server_with_db(db).await;
    insert_test_stock(&db, "DEMO-A", "架空工業").await;
    let (note_id, first_version_id) = create_note(&server, RAW_BODY).await;
    let second_response = server
        .patch(&format!("/api/notes/{note_id}"))
        .json(&json!({ "title": "更新済みの分析ノート" }))
        .await;
    let second_body = second_response.json::<Value>();
    assert_eq!(
        (
            second_response.status_code(),
            normalize_note_response(second_body)
        ),
        (
            StatusCode::OK,
            json!({
                "id": "<id>",
                "version_id": "<version_id>",
                "version_no": 2,
                "is_current": true,
                "title": "更新済みの分析ノート",
                "body_md": RAW_BODY,
                "frontmatter_json": {},
                "resolved_price_references_json": {},
                "tags": [],
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
    let created_comment = server
        .post("/api/comments")
        .json(&json!({
            "target_kind": "note_version",
            "target_id": first_version_id,
            "body": "架空のレビューコメント",
            "anchor_text": "続く行",
            "anchor_side": "new",
            "start_line": 2,
            "end_line": 2,
        }))
        .await;
    let created_comment_body = created_comment.json::<Value>();
    assert_eq!(
        (
            created_comment.status_code(),
            normalize_comment(created_comment_body)
        ),
        (
            StatusCode::CREATED,
            json!({
                "id": "<id>",
                "target_kind": "note_version",
                "target_id": first_version_id,
                "parent_id": null,
                "body": "架空のレビューコメント",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "続く行",
                "anchor_side": "new",
                "start_line": 2,
                "end_line": 2,
            }),
        ),
    );

    let versions_before = get_versions(&server, note_id).await;
    let comments_before = get_comments(&server, first_version_id).await;
    let versions_before_normalized = normalize_note_versions(versions_before.clone());
    let first_detail = server
        .get(&format!(
            "/api/notes/{note_id}?version_id={first_version_id}"
        ))
        .await;
    let first_detail_body = first_detail.json::<Value>();
    let first_detail_body = normalize_note_response(first_detail_body);
    let repeated_first_detail = server
        .get(&format!(
            "/api/notes/{note_id}?version_id={first_version_id}"
        ))
        .await;
    let repeated_first_detail_body = normalize_note_response(repeated_first_detail.json::<Value>());
    let current_detail = server.get(&format!("/api/notes/{note_id}")).await;
    let current_detail_body = normalize_note_response(current_detail.json::<Value>());
    let versions_after = get_versions(&server, note_id).await;
    let comments_after = get_comments(&server, first_version_id).await;

    assert_eq!(
        (
            first_detail.status_code(),
            first_detail_body,
            repeated_first_detail_body,
            current_detail.status_code(),
            current_detail_body,
            versions_before_normalized.clone(),
            normalize_note_versions(versions_after.clone()),
            versions_before == versions_after,
            comments_before == comments_after,
            (RAW_BODY.lines().count(), DISPLAY_BODY.lines().count()),
            normalize_comments(comments_after),
        ),
        (
            StatusCode::OK,
            expected_detail(
                "<version_id>",
                1,
                false,
                "架空の分析ノート",
                RAW_BODY,
                DISPLAY_BODY,
            ),
            expected_detail(
                "<version_id>",
                1,
                false,
                "架空の分析ノート",
                RAW_BODY,
                DISPLAY_BODY,
            ),
            StatusCode::OK,
            expected_detail(
                "<version_id>",
                2,
                true,
                "更新済みの分析ノート",
                RAW_BODY,
                DISPLAY_BODY,
            ),
            versions_before_normalized.clone(),
            versions_before_normalized,
            true,
            true,
            (2, 2),
            json!([
                {
                    "id": "<id>",
                    "target_kind": "note_version",
                    "target_id": first_version_id,
                    "parent_id": null,
                    "body": "架空のレビューコメント",
                    "author_kind": "human",
                    "author_label": "user",
                    "resolved": false,
                    "created_at": "<created_at>",
                    "anchor_text": "続く行",
                    "anchor_side": "new",
                    "start_line": 2,
                    "end_line": 2,
                }
            ]),
        ),
    );
}

#[backend_test_macros::database_test]
async fn get_note_keeps_an_unknown_stock_link_unchanged(db: DatabaseHandle) {
    let (_, server) = create_test_server_with_db(db).await;
    let raw_body = "[[stock:DEMO-UNKNOWN]] 架空銘柄: 内容";
    let (note_id, version_id) = create_note(&server, raw_body).await;
    let response = server
        .get(&format!("/api/notes/{note_id}?version_id={version_id}"))
        .await;

    assert_eq!(
        (
            response.status_code(),
            normalize_note_response(response.json::<Value>()),
        ),
        (
            StatusCode::OK,
            expected_detail(
                "<version_id>",
                1,
                true,
                "架空の分析ノート",
                raw_body,
                raw_body,
            ),
        ),
    );
}

struct FailingStockNameRepository;

#[async_trait]
impl RefRepository for FailingStockNameRepository {
    async fn list_stocks(&self, _: Option<&str>) -> Result<Vec<StockRef>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn find_stock(&self, _: &str) -> Result<Option<StockRef>, RefRepositoryError> {
        Ok(None)
    }

    async fn stock_sectors(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, Option<String>>, RefRepositoryError> {
        Ok(HashMap::new())
    }

    async fn list_indicators(
        &self,
        _: Option<&str>,
    ) -> Result<Vec<IndicatorRef>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn find_indicator(&self, _: &str) -> Result<Option<IndicatorRef>, RefRepositoryError> {
        Ok(None)
    }

    async fn search_all(&self, _: &str, _: u64) -> Result<Vec<RefSearchMatch>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn stock_names(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        Err(RefRepositoryError::Database(PersistenceError::Database(
            "架空の参照解決エラー".into(),
        )))
    }

    async fn indicator_names(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        Ok(HashMap::new())
    }

    async fn group_names(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        Ok(HashMap::new())
    }

    async fn list_terms(&self, _: &str) -> Result<Vec<RefTerm>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn insert_term(
        &self,
        _: &UnitOfWorkTransaction,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<bool, RefRepositoryError> {
        Ok(false)
    }

    async fn delete_term(
        &self,
        _: &UnitOfWorkTransaction,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<bool, RefRepositoryError> {
        Ok(false)
    }
}

fn note_reads_with_failing_stock_names(db: DatabaseHandle) -> NoteReadUseCases {
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let repository: SharedRefRepository = Arc::new(FailingStockNameRepository);
    let refs = RefUseCases::new(unit_of_work, repository);
    NoteReadUseCases::with_markdown_lint(
        Arc::new(PostgresNoteReadQuery::new(db)),
        MarkdownLintService::new(refs),
    )
}

#[backend_test_macros::database_test]
async fn get_note_falls_back_to_raw_body_when_stock_name_resolution_fails(db: DatabaseHandle) {
    let (db, create_server) = create_test_server_with_db(db).await;
    let (note_id, version_id) = create_note(&create_server, RAW_BODY).await;
    let server =
        create_test_server_with_note_reads(db.clone(), note_reads_with_failing_stock_names(db))
            .await;
    let response = server
        .get(&format!("/api/notes/{note_id}?version_id={version_id}"))
        .await;

    assert_eq!(
        (
            response.status_code(),
            normalize_note_response(response.json::<Value>()),
        ),
        (
            StatusCode::OK,
            expected_detail(
                "<version_id>",
                1,
                true,
                "架空の分析ノート",
                RAW_BODY,
                RAW_BODY,
            ),
        ),
    );
}
