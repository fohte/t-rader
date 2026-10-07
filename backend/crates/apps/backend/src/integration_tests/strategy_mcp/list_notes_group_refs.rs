use core_application::group_axis::CreateGroupAxisCommand;
use core_application::stock_group::CreateStockGroupCommand;
use gateway_postgres::entities::stock;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use uuid::Uuid;

use super::dto::{ListNotesParams, ListNotesResult, WriteNoteParams};
use super::test_server::ToolOutput;
use super::tests_common::{build_server, insert_note_kind, insert_strategy};

async fn insert_stock(db: &gateway_postgres::DatabaseHandle, stock_id: &str) {
    stock::ActiveModel {
        id: Set(stock_id.into()),
        name: Set("Sample stock".into()),
        market: Set(None),
        product_category: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert stock");
}

async fn write_note(
    server: &super::test_server::StrategyServer,
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
    kind: Option<Option<String>>,
) -> Uuid {
    server
        .write_note(
            strategy_id,
            None,
            WriteNoteParams {
                note_id: None,
                title: Some(title.into()),
                body_md: Some(body_md.into()),
                kind,
                change_reason: None,
                frontmatter_json: None,
                graphs: None,
            },
        )
        .await
        .expect("write note")
        .note_id
}

fn sorted_note_ids(result: ToolOutput<ListNotesResult>) -> Vec<Uuid> {
    let mut ids = result
        .into_value()
        .notes
        .into_iter()
        .map(|note| note.note_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

#[backend_test_macros::database_test]
async fn stock_ref_includes_notes_for_every_member_group_without_duplicates(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "sample-strategy").await;
    insert_note_kind(&db, "sample-review-kind", true).await;
    insert_stock(&db, "demo-code").await;

    let use_cases = crate::services::use_cases::build_use_cases(db.clone());
    let group_axes = use_cases.group_axes();
    let stock_groups = use_cases.stock_groups();
    for (axis_key, group_key) in [
        ("demo-axis", "demo-group"),
        ("sample-axis", "sample-group"),
        ("other-axis", "other-group"),
    ] {
        group_axes
            .create(CreateGroupAxisCommand {
                key: axis_key.into(),
                name: "Sample axis".into(),
                description: "Sample axis".into(),
                derive_from: None,
            })
            .await
            .expect("create group axis");
        stock_groups
            .create(CreateStockGroupCommand {
                axis_key: axis_key.into(),
                key: group_key.into(),
                name: "Sample group".into(),
                description: None,
            })
            .await
            .expect("create stock group");
    }
    for (axis_key, group_key) in [("demo-axis", "demo-group"), ("sample-axis", "sample-group")] {
        stock_groups
            .add_stock(axis_key, group_key, "demo-code")
            .await
            .expect("add stock to group");
    }

    let server = build_server(db);
    let direct_note_id = write_note(
        &server,
        strategy_id,
        "Direct reference",
        "[[stock:demo-code]]",
        None,
    )
    .await;
    let first_group_note_id = write_note(
        &server,
        strategy_id,
        "First group reference",
        "[[group:demo-axis/demo-group]]",
        None,
    )
    .await;
    let second_group_note_id = write_note(
        &server,
        strategy_id,
        "Second group reference",
        "[[group:sample-axis/sample-group]]",
        None,
    )
    .await;
    let overlapping_note_id = write_note(
        &server,
        strategy_id,
        "Multiple matching references",
        "[[stock:demo-code]] [[group:demo-axis/demo-group]] [[group:sample-axis/sample-group]]",
        None,
    )
    .await;
    let pending_note_id = write_note(
        &server,
        strategy_id,
        "Pending group reference",
        "[[group:sample-axis/sample-group]]",
        Some(Some("sample-review-kind".into())),
    )
    .await;
    write_note(
        &server,
        strategy_id,
        "Unrelated group reference",
        "[[group:other-axis/other-group]]",
        None,
    )
    .await;

    let current = server
        .list_notes(
            strategy_id,
            ListNotesParams {
                r#ref: Some("stock:demo-code".into()),
                ..Default::default()
            },
        )
        .await
        .expect("list current notes by stock reference");
    let including_pending = server
        .list_notes(
            strategy_id,
            ListNotesParams {
                r#ref: Some("stock:demo-code".into()),
                include_pending: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("list pending notes by stock reference");
    let group_reference = server
        .list_notes(
            strategy_id,
            ListNotesParams {
                r#ref: Some("group:demo-axis/demo-group".into()),
                ..Default::default()
            },
        )
        .await
        .expect("list notes by group reference");

    let mut expected_current = vec![
        direct_note_id,
        first_group_note_id,
        second_group_note_id,
        overlapping_note_id,
    ];
    expected_current.sort_unstable();
    let mut expected_including_pending = expected_current.clone();
    expected_including_pending.push(pending_note_id);
    expected_including_pending.sort_unstable();
    let mut expected_group_reference = vec![first_group_note_id, overlapping_note_id];
    expected_group_reference.sort_unstable();

    assert_eq!(
        (
            sorted_note_ids(current),
            sorted_note_ids(including_pending),
            sorted_note_ids(group_reference),
        ),
        (
            expected_current,
            expected_including_pending,
            expected_group_reference,
        ),
    );
}
