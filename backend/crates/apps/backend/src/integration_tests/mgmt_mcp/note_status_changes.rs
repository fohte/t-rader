use std::sync::Arc;

use chrono::{DateTime, FixedOffset};
use core_application::agent_task_client::FakeAgentTaskClient;
use gateway_postgres::entities::change_history;
use rmcp::handler::server::wrapper::{Json, Parameters};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::json;
use uuid::Uuid;

use super::dto::{GetNoteStatusChangeCountsParams, GetNoteStatusChangeCountsResult};
use super::tests_common::build_server;

fn timestamp(value: &str) -> DateTime<FixedOffset> {
    value.parse().expect("timestamp")
}

async fn insert_history(
    db: &gateway_postgres::DatabaseHandle,
    target_kind: &str,
    op: &str,
    created_at: &str,
    to: &str,
) {
    change_history::ActiveModel {
        id: Set(Uuid::new_v4()),
        target_kind: Set(target_kind.into()),
        target_id: Set(Uuid::new_v4()),
        actor_kind: Set("human".into()),
        actor_label: Set("user".into()),
        op: Set(op.into()),
        diff_json: Set(json!({ "to": to })),
        summary: Set(None),
        created_at: Set(timestamp(created_at)),
    }
    .insert(db)
    .await
    .expect("insert change history");
}

#[backend_test_macros::database_test]
async fn status_change_counts_filter_by_note_operation_destination_and_period(
    db: gateway_postgres::DatabaseHandle,
) {
    insert_history(
        &db,
        "note",
        "status_change",
        "2026-09-01T00:00:00Z",
        "approved",
    )
    .await;
    insert_history(
        &db,
        "note",
        "status_change",
        "2026-09-15T00:00:00Z",
        "rejected",
    )
    .await;
    insert_history(
        &db,
        "note",
        "status_change",
        "2026-08-31T23:59:59Z",
        "rejected",
    )
    .await;
    insert_history(
        &db,
        "note",
        "status_change",
        "2026-10-01T00:00:00Z",
        "approved",
    )
    .await;
    insert_history(
        &db,
        "annotation",
        "status_change",
        "2026-09-15T00:00:00Z",
        "rejected",
    )
    .await;
    insert_history(&db, "note", "update", "2026-09-15T00:00:00Z", "approved").await;
    insert_history(
        &db,
        "note",
        "status_change",
        "2026-09-15T00:00:00Z",
        "unread",
    )
    .await;

    let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
    let Json(result) = server
        .get_note_status_change_counts(Parameters(GetNoteStatusChangeCountsParams {
            from: timestamp("2026-09-01T00:00:00Z"),
            to: timestamp("2026-10-01T00:00:00Z"),
        }))
        .await
        .expect("count status changes");

    assert_eq!(
        serde_json::to_value(result).expect("serialize result"),
        serde_json::to_value(GetNoteStatusChangeCountsResult {
            from: timestamp("2026-09-01T00:00:00Z"),
            to: timestamp("2026-10-01T00:00:00Z"),
            approved_count: 1,
            rejected_count: 1,
        })
        .expect("serialize expected result"),
    );
}

#[backend_test_macros::database_test]
async fn status_change_counts_rejects_empty_period(db: gateway_postgres::DatabaseHandle) {
    let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
    let result = server
        .get_note_status_change_counts(Parameters(GetNoteStatusChangeCountsParams {
            from: timestamp("2026-09-01T00:00:00Z"),
            to: timestamp("2026-09-01T00:00:00Z"),
        }))
        .await;

    assert_eq!(
        result.err(),
        Some(rmcp::ErrorData::invalid_params(
            "to must be later than from",
            None,
        )),
    );
}
