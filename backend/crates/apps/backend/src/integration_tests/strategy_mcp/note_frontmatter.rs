use serde_json::json;

use super::dto::{ListNotesParams, WriteNoteParams};
use super::tests_common::{build_server, insert_strategy};

#[backend_test_macros::database_test]
async fn write_note_rejects_tags_that_are_not_a_string_array(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "tag-validation").await;
    let server = build_server(db.clone());

    let error = server
        .write_note(
            strategy_id,
            None,
            WriteNoteParams {
                note_id: None,
                title: Some("sample note".into()),
                body_md: None,
                kind: None,
                frontmatter_json: Some(
                    json!({"tags": "sample-label"}).as_object().unwrap().clone(),
                ),
                change_reason: None,
                graphs: None,
            },
        )
        .await
        .expect_err("tags must be rejected unless they are a string array");
    let notes = server
        .list_notes(strategy_id, ListNotesParams::default())
        .await
        .expect("list notes");

    assert_eq!(
        (error.code, error.message.to_string(), notes.notes.len()),
        (
            rmcp::model::ErrorCode::INVALID_PARAMS,
            "frontmatter_json.tags must be an array of strings".to_string(),
            0,
        ),
    );
}
