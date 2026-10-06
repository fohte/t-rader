use std::sync::Arc;

use crate::change_history::{Actor, FakeChangeHistory, Op, TargetKind};
use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};
use rstest::{fixture, rstest};
use serde_json::json;
use uuid::Uuid;

use super::{
    CreateStockGroupCommand, FakeStockGroupRepository, StockGroup, StockGroupMembership,
    StockGroupSyncSourceCodeLookup, StockGroupUseCaseError, StockGroupUseCases,
    UpdateStockGroupCommand,
};
use crate::stock_group::SharedStockGroupRepository;

struct Harness {
    use_cases: StockGroupUseCases,
    repository: Arc<FakeStockGroupRepository>,
    unit_of_work: Arc<FakeUnitOfWork>,
    change_history: Arc<FakeChangeHistory>,
}

#[fixture]
fn harness() -> Harness {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeStockGroupRepository::new());
    let change_history = Arc::new(FakeChangeHistory::new());
    let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
    let shared_repository: SharedStockGroupRepository = repository.clone();
    let shared_change_history = change_history.clone();

    Harness {
        use_cases: StockGroupUseCases::new(
            shared_unit_of_work,
            shared_repository,
            shared_change_history,
        ),
        repository,
        unit_of_work,
        change_history,
    }
}

async fn create_group(harness: &Harness) -> Result<StockGroup, StockGroupUseCaseError> {
    harness.repository.insert_axis("sample-axis", None).await;
    harness
        .use_cases
        .create(CreateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: "Sample group".into(),
            description: Some("Sample description".into()),
        })
        .await
}

#[rstest]
#[tokio::test]
async fn source_code_lookup_distinguishes_missing_and_ambiguous_groups(harness: Harness) {
    harness
        .repository
        .insert_axis("sample-axis-a", Some("synthetic-source"))
        .await;
    harness
        .repository
        .insert_axis("sample-axis-b", Some("synthetic-source"))
        .await;
    harness
        .repository
        .insert_sync_group("sample-axis-a", "sample-coded", Some("1234"))
        .await;
    harness
        .repository
        .insert_sync_group("sample-axis-a", "sample-pending", None)
        .await;
    harness
        .repository
        .insert_sync_group("sample-axis-a", "sample-conflict", Some("1234"))
        .await;
    harness
        .repository
        .insert_sync_group("sample-axis-b", "sample-conflict", Some("5678"))
        .await;

    let results = vec![
        harness
            .use_cases
            .find_sync_source_code("synthetic-source", "sample-unknown")
            .await,
        harness
            .use_cases
            .find_sync_source_code("synthetic-source", "sample-pending")
            .await,
        harness
            .use_cases
            .find_sync_source_code("synthetic-source", "sample-coded")
            .await,
        harness
            .use_cases
            .find_sync_source_code("synthetic-source", "sample-conflict")
            .await,
    ];
    let results = results
        .into_iter()
        .map(|result| result.map_err(|error| error.to_string()))
        .collect::<Vec<_>>();

    assert_eq!(
        results,
        vec![
            Ok(StockGroupSyncSourceCodeLookup::NotFound),
            Ok(StockGroupSyncSourceCodeLookup::Missing),
            Ok(StockGroupSyncSourceCodeLookup::Found("1234".into())),
            Ok(StockGroupSyncSourceCodeLookup::Ambiguous),
        ],
    );
}

#[rstest]
#[tokio::test]
async fn create_records_history_in_the_same_transaction(harness: Harness) {
    harness.repository.insert_axis("sample-axis", None).await;
    let group = harness
        .use_cases
        .create(CreateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: "Sample group".into(),
            description: Some("Sample description".into()),
        })
        .await
        .expect("group creation succeeds");
    let transaction_id = harness
        .unit_of_work
        .committed
        .lock()
        .await
        .first()
        .copied()
        .unwrap_or_default();
    let committed = harness.unit_of_work.committed.lock().await.clone();
    let repository_transactions = harness.repository.transaction_ids.lock().await.clone();
    let history = harness.change_history.entries.lock().await.clone();

    assert_eq!(
        (
            group.clone(),
            committed,
            repository_transactions
                .iter()
                .map(|id| *id == transaction_id)
                .collect::<Vec<_>>(),
            history
                .into_iter()
                .map(|entry| (
                    entry.transaction_id == transaction_id,
                    entry.record.actor,
                    entry.record.target_kind,
                    entry.record.target_id,
                    entry.record.op,
                    entry.record.diff,
                    entry.record.summary,
                ))
                .collect::<Vec<_>>(),
        ),
        (
            StockGroup {
                id: group.id,
                axis_id: group.axis_id,
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: "Sample group".into(),
                description: Some("Sample description".into()),
            },
            vec![transaction_id],
            vec![true; 3],
            vec![(
                true,
                Actor::Llm { label: "analyst" },
                TargetKind::StockGroup,
                group.id,
                Op::Create,
                json!({
                    "axis_key": "sample-axis",
                    "group_key": "sample-group",
                    "name": "Sample group",
                    "description": "Sample description",
                }),
                None,
            )],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn create_maps_repository_conflict_to_validation(harness: Harness) {
    harness.repository.insert_axis("sample-axis", None).await;
    harness
        .repository
        .conflict_next_insert("unique constraint conflict")
        .await;

    let error = harness
        .use_cases
        .create(CreateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: "Sample group".into(),
            description: None,
        })
        .await
        .expect_err("repository conflict is returned as validation");
    let history = harness.change_history.entries.lock().await.clone();
    let committed = harness.unit_of_work.committed.lock().await.clone();

    assert_eq!(
        (error.to_string(), history.len(), committed.len()),
        (
            "stock group sample-axis/sample-group already exists".to_string(),
            0,
            0,
        ),
    );
}

#[rstest]
#[tokio::test]
async fn update_records_only_changed_fields_and_keeps_keys_immutable(harness: Harness) {
    let group = create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.change_history.entries.lock().await.clear();
    let updated = harness
        .use_cases
        .update(UpdateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: Some("Renamed group".into()),
            description: Some(None),
        })
        .await
        .expect("group update succeeds");
    let history = harness.change_history.entries.lock().await.clone();

    assert_eq!(
        (
            updated,
            history
                .into_iter()
                .map(|entry| (
                    entry.record.actor,
                    entry.record.target_kind,
                    entry.record.target_id,
                    entry.record.op,
                    entry.record.diff,
                ))
                .collect::<Vec<_>>(),
        ),
        (
            StockGroup {
                id: group.id,
                axis_id: group.axis_id,
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: "Renamed group".into(),
                description: None,
            },
            vec![(
                Actor::Llm { label: "analyst" },
                TargetKind::StockGroup,
                group.id,
                Op::Update,
                json!({
                    "description": { "from": "Sample description", "to": null },
                    "name": { "from": "Sample group", "to": "Renamed group" },
                }),
            )],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn add_stock_is_idempotent_and_audited(harness: Harness) {
    let group = create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.change_history.entries.lock().await.clear();
    harness.repository.insert_stock("0002").await;

    let added_first = harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("first add succeeds");
    let added_second = harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("repeated add succeeds");

    let history = stock_group_history(&harness).await;

    assert_eq!(
        (added_first, added_second, history),
        (
            true,
            false,
            vec![(
                Actor::Llm { label: "analyst" },
                TargetKind::StockGroup,
                group.id,
                Op::Update,
                json!({
                    "stock_id": "0002",
                    "membership": { "from": false, "to": true },
                }),
            )],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn add_stock_rejects_unknown_stock(harness: Harness) {
    create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.change_history.entries.lock().await.clear();
    harness.unit_of_work.committed.lock().await.clear();

    let error = harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0009")
        .await
        .expect_err("unknown stock is rejected");
    let history = harness.change_history.entries.lock().await.clone();
    let committed = harness.unit_of_work.committed.lock().await.clone();

    assert_eq!(
        (error.to_string(), history.len(), committed.len()),
        ("stock 0009 not found".to_string(), 0, 0),
    );
}

#[rstest]
#[tokio::test]
async fn list_stock_ids_returns_sorted_members(harness: Harness) {
    create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.repository.insert_stock("0001").await;
    harness.repository.insert_stock("0002").await;
    harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("first stock add succeeds");
    harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0001")
        .await
        .expect("second stock add succeeds");

    let stocks = harness
        .use_cases
        .list_stock_ids("sample-axis", "sample-group")
        .await
        .expect("stock listing succeeds");

    assert_eq!(stocks, vec!["0001".to_string(), "0002".to_string()]);
}

#[rstest]
#[tokio::test]
async fn remove_stock_is_idempotent_and_audited(harness: Harness) {
    let group = create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.repository.insert_stock("0002").await;
    harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("stock add succeeds");
    harness.change_history.entries.lock().await.clear();

    let removed_first = harness
        .use_cases
        .remove_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("first remove succeeds");
    let removed_second = harness
        .use_cases
        .remove_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("repeated remove succeeds");
    let remaining = harness
        .use_cases
        .list_stock_ids("sample-axis", "sample-group")
        .await
        .expect("stock listing succeeds");
    let history = stock_group_history(&harness).await;

    assert_eq!(
        (removed_first, removed_second, remaining, history),
        (
            true,
            false,
            Vec::new(),
            vec![(
                Actor::Llm { label: "analyst" },
                TargetKind::StockGroup,
                group.id,
                Op::Update,
                json!({
                    "stock_id": "0002",
                    "membership": { "from": true, "to": false },
                }),
            ),],
        ),
    );
}

async fn stock_group_history(
    harness: &Harness,
) -> Vec<(Actor, TargetKind, Uuid, Op, serde_json::Value)> {
    harness
        .change_history
        .entries
        .lock()
        .await
        .iter()
        .map(|entry| {
            (
                entry.record.actor,
                entry.record.target_kind,
                entry.record.target_id,
                entry.record.op,
                entry.record.diff.clone(),
            )
        })
        .collect()
}

#[rstest]
#[tokio::test]
async fn synchronized_axes_reject_group_and_membership_mutations(harness: Harness) {
    harness
        .repository
        .insert_axis("sample-axis", Some("sample-sync"))
        .await;

    let errors = vec![
        harness
            .use_cases
            .create(CreateStockGroupCommand {
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: "Sample group".into(),
                description: None,
            })
            .await
            .expect_err("synchronized axis rejects group creation")
            .to_string(),
        harness
            .use_cases
            .update(UpdateStockGroupCommand {
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: None,
            })
            .await
            .expect_err("synchronized axis rejects group update")
            .to_string(),
        harness
            .use_cases
            .add_stock("sample-axis", "sample-group", "0001")
            .await
            .expect_err("synchronized axis rejects member addition")
            .to_string(),
        harness
            .use_cases
            .remove_stock("sample-axis", "sample-group", "0001")
            .await
            .expect_err("synchronized axis rejects member removal")
            .to_string(),
    ];

    assert_eq!(
        (
            errors,
            harness.change_history.entries.lock().await.len(),
            harness.unit_of_work.committed.lock().await.len(),
        ),
        (
            vec!["stock groups on synchronized axes cannot be changed by MCP".to_string(); 4],
            0,
            0,
        ),
    );
}

#[rstest]
#[tokio::test]
async fn list_memberships_returns_every_group_for_requested_stocks_and_axes(harness: Harness) {
    let first_group = create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.repository.insert_axis("other-axis", None).await;
    let second_group = harness
        .use_cases
        .create(CreateStockGroupCommand {
            axis_key: "other-axis".into(),
            key: "other-group".into(),
            name: "Other group".into(),
            description: None,
        })
        .await
        .expect("group creation succeeds");
    harness.repository.insert_stock("demo-stock").await;
    harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "demo-stock")
        .await
        .expect("stock membership succeeds");
    harness
        .use_cases
        .add_stock("other-axis", "other-group", "demo-stock")
        .await
        .expect("stock membership succeeds");

    let memberships = harness
        .use_cases
        .list_memberships(
            &["demo-stock".to_string(), "unrelated-stock".to_string()],
            Some(&["sample-axis".to_string(), "other-axis".to_string()]),
        )
        .await
        .expect("membership listing succeeds");

    assert_eq!(
        memberships,
        vec![
            StockGroupMembership {
                axis_key: "other-axis".into(),
                group_key: second_group.key,
                stock_id: "demo-stock".into(),
            },
            StockGroupMembership {
                axis_key: "sample-axis".into(),
                group_key: first_group.key,
                stock_id: "demo-stock".into(),
            },
        ],
    );
}
